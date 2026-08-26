use alloc::{boxed::Box, format, vec, vec::Vec};

use arm32_cpu::{Cpu, Memory, Mode, reg};

use wie_util::{Result, WieError};

use crate::engine::{ArmEngine, ArmRegister, EngineRunResult, MemoryPermission};

const INOTIA1_EXP_DIAG_EVENT_LIMIT: u32 = 600;
const INOTIA1_EXP_DIAG_REPEAT_LIMIT: u8 = 4;
const INOTIA1_EXP_DIAG_CALLSITE_LIMIT: u8 = 24;
// Phase 8.45 field data identified this exact STRH callsite as a hot RGB565/
// pixel-buffer writer: 596/600 retained events came from it within ~76 ms.
// Suppress only the observed 16-bit writer, not its destination heap region,
// because real player/monster structures may also live in 0x4xxxxxxx memory.
const INOTIA1_EXP_DIAG_NOISY_RGB565_PC: u32 = 0x0010_69c2;
// Phase 8.48 field confirmation: the user's displayed EXP 97,567 is
// 0x00017d1f and appears repeatedly as word 0 of the live main-character
// object at 0x00171040. Exact writes touching this 4-byte word are logged
// independently from the broad candidate budget and callsite caps.
const INOTIA1_MAIN_EXP_ADDR: u32 = 0x0017_1040;
// Phase 8.48 field data shows the Inotia1 live entity array is laid out from
// the main-character object at 0x00171040 with a 0x424-byte stride. Slot 6
// (0x00172918) held 수호자 C44 base reward +3172, while slot 7
// (0x00172d3c) held 수호물 K34 base reward -3035 before the final EXP
// distribution routine. Watch word 0 of the first 32 entity slots so the
// next map/spawn test identifies the instruction that creates those signed
// base-reward values. Slot 0 is the player's EXP and retains its dedicated
// exact watchpoint above; entity-reward tracing starts at slot 1.
const INOTIA1_ENTITY_BASE_ADDR: u32 = INOTIA1_MAIN_EXP_ADDR;
const INOTIA1_ENTITY_STRIDE: u32 = 0x424;
const INOTIA1_ENTITY_SLOT_COUNT: u32 = 32;
const INOTIA1_ENTITY_REWARD_EVENT_LIMIT: u32 = 512;

#[derive(Clone, Copy)]
struct Inotia1ExpCandidate {
    addr: u32,
    old: u32,
    new: u32,
    width_bits: u8,
    pc_before: u32,
}

#[derive(Clone, Copy)]
struct Inotia1ExactExpWrite {
    store_addr: u32,
    store_size_bytes: u32,
    old_exp: u32,
    new_exp: u32,
    pc_before: u32,
}

#[derive(Clone, Copy)]
struct Inotia1EntityRewardWrite {
    slot: u32,
    field_addr: u32,
    store_addr: u32,
    store_size_bytes: u32,
    old_value: u32,
    new_value: u32,
    pc_before: u32,
}

#[derive(Clone, Copy)]
struct Inotia1ExpSeenWrite {
    addr: u32,
    pc_before: u32,
    width_bits: u8,
    count: u8,
}

#[derive(Clone, Copy)]
struct Inotia1ExpSeenCallsite {
    pc_before: u32,
    width_bits: u8,
    count: u8,
}

pub struct Arm32CpuEngine {
    cpu: Cpu,
    mem: EmulatedMemory,
    inotia1_exp_diag_enabled: bool,
    inotia1_exp_diag_events: u32,
    inotia1_exp_diag_saturated: bool,
    inotia1_exp_diag_seen: Vec<Inotia1ExpSeenWrite>,
    inotia1_exp_diag_callsites: Vec<Inotia1ExpSeenCallsite>,
    inotia1_exact_exp_events: u32,
    inotia1_entity_reward_events: u32,
    inotia1_entity_reward_saturated: bool,
}

impl Arm32CpuEngine {
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            mem: EmulatedMemory::new(),
            inotia1_exp_diag_enabled: false,
            inotia1_exp_diag_events: 0,
            inotia1_exp_diag_saturated: false,
            inotia1_exp_diag_seen: Vec::new(),
            inotia1_exp_diag_callsites: Vec::new(),
            inotia1_exact_exp_events: 0,
            inotia1_entity_reward_events: 0,
            inotia1_entity_reward_saturated: false,
        }
    }

    fn trace_inotia1_exact_exp_write(&mut self, write: Inotia1ExactExpWrite) {
        self.inotia1_exact_exp_events = self.inotia1_exact_exp_events.saturating_add(1);
        let event = self.inotia1_exact_exp_events;
        let pc_after = self.cpu.reg_get(Mode::User, reg::PC);
        let lr = self.cpu.reg_get(Mode::User, reg::LR);
        let sp = self.cpu.reg_get(Mode::User, reg::SP);
        let regs = [
            self.cpu.reg_get(Mode::User, 0),
            self.cpu.reg_get(Mode::User, 1),
            self.cpu.reg_get(Mode::User, 2),
            self.cpu.reg_get(Mode::User, 3),
            self.cpu.reg_get(Mode::User, 4),
            self.cpu.reg_get(Mode::User, 5),
            self.cpu.reg_get(Mode::User, 6),
            self.cpu.reg_get(Mode::User, 7),
            self.cpu.reg_get(Mode::User, 8),
            self.cpu.reg_get(Mode::User, 9),
            self.cpu.reg_get(Mode::User, 10),
            self.cpu.reg_get(Mode::User, 11),
            self.cpu.reg_get(Mode::User, 12),
        ];
        let delta = write.new_exp as i64 - write.old_exp as i64;
        let delta_low12 = (delta as i32 as u32) & 0x0fff;
        let delta_s12 = Self::sign_extend_12(delta_low12);

        tracing::info!(
            "[PHASE8_48_INOTIA1_EXP_EXACT_WRITE] event={event} exp_addr={:#010x} store_addr={:#010x} store_size={} old={} new={} delta={:+} old_hex={:#010x} new_hex={:#010x} delta_low12={:#05x} delta_s12={} pc_before={:#010x} pc_after={:#010x} lr={:#010x} sp={:#010x} r0={:#010x} r1={:#010x} r2={:#010x} r3={:#010x} r4={:#010x} r5={:#010x} r6={:#010x} r7={:#010x} r8={:#010x} r9={:#010x} r10={:#010x} r11={:#010x} r12={:#010x}",
            INOTIA1_MAIN_EXP_ADDR,
            write.store_addr,
            write.store_size_bytes,
            write.old_exp,
            write.new_exp,
            delta,
            write.old_exp,
            write.new_exp,
            delta_low12,
            delta_s12,
            write.pc_before,
            pc_after,
            lr,
            sp,
            regs[0], regs[1], regs[2], regs[3], regs[4], regs[5], regs[6], regs[7], regs[8], regs[9], regs[10], regs[11], regs[12],
        );

        let code_base = write.pc_before.saturating_sub(24) & !1;
        let mut code = [0u8; 64];
        if self.mem.read_range(code_base, code.len(), &mut code).is_ok() {
            tracing::info!(
                "[PHASE8_48_INOTIA1_EXP_EXACT_CODE] event={event} base={code_base:#010x} bytes={code:02x?}"
            );
        }

        let stack_base = sp.saturating_sub(32) & !3;
        let mut stack = [0u32; 24];
        let mut stack_ok = true;
        for (index, word) in stack.iter_mut().enumerate() {
            let address = stack_base.wrapping_add(index as u32 * 4);
            let mut bytes = [0u8; 4];
            if self.mem.read_range(address, 4, &mut bytes).is_err() {
                stack_ok = false;
                break;
            }
            *word = u32::from_le_bytes(bytes);
        }
        if stack_ok {
            tracing::info!(
                "[PHASE8_48_INOTIA1_EXP_EXACT_STACK] event={event} base={stack_base:#010x} words={stack:08x?}"
            );
        }

        // The field result -1179 is exactly the signed-12 interpretation of
        // 0xB65. Print each live register's low 12 bits and signed-12 value so
        // the suspected packed-value/sign-extension defect is directly testable
        // at the exact EXP store without hard-coding any monster reward.
        let reg_low12 = [
            regs[0] & 0x0fff, regs[1] & 0x0fff, regs[2] & 0x0fff, regs[3] & 0x0fff,
            regs[4] & 0x0fff, regs[5] & 0x0fff, regs[6] & 0x0fff, regs[7] & 0x0fff,
            regs[8] & 0x0fff, regs[9] & 0x0fff, regs[10] & 0x0fff, regs[11] & 0x0fff,
            regs[12] & 0x0fff,
        ];
        let reg_s12 = [
            Self::sign_extend_12(reg_low12[0]), Self::sign_extend_12(reg_low12[1]),
            Self::sign_extend_12(reg_low12[2]), Self::sign_extend_12(reg_low12[3]),
            Self::sign_extend_12(reg_low12[4]), Self::sign_extend_12(reg_low12[5]),
            Self::sign_extend_12(reg_low12[6]), Self::sign_extend_12(reg_low12[7]),
            Self::sign_extend_12(reg_low12[8]), Self::sign_extend_12(reg_low12[9]),
            Self::sign_extend_12(reg_low12[10]), Self::sign_extend_12(reg_low12[11]),
            Self::sign_extend_12(reg_low12[12]),
        ];
        tracing::info!(
            "[PHASE8_48_INOTIA1_EXP_LOW12_SIGNATURE] event={event} delta={:+} r0={:#05x}/{} r1={:#05x}/{} r2={:#05x}/{} r3={:#05x}/{} r4={:#05x}/{} r5={:#05x}/{} r6={:#05x}/{} r7={:#05x}/{} r8={:#05x}/{} r9={:#05x}/{} r10={:#05x}/{} r11={:#05x}/{} r12={:#05x}/{}",
            delta,
            reg_low12[0], reg_s12[0], reg_low12[1], reg_s12[1], reg_low12[2], reg_s12[2],
            reg_low12[3], reg_s12[3], reg_low12[4], reg_s12[4], reg_low12[5], reg_s12[5],
            reg_low12[6], reg_s12[6], reg_low12[7], reg_s12[7], reg_low12[8], reg_s12[8],
            reg_low12[9], reg_s12[9], reg_low12[10], reg_s12[10], reg_low12[11], reg_s12[11],
            reg_low12[12], reg_s12[12],
        );

        let mut player_words = [0u32; 24];
        let mut player_ok = true;
        for (index, word) in player_words.iter_mut().enumerate() {
            let address = INOTIA1_MAIN_EXP_ADDR.wrapping_add(index as u32 * 4);
            let mut bytes = [0u8; 4];
            if self.mem.read_range(address, 4, &mut bytes).is_err() {
                player_ok = false;
                break;
            }
            *word = u32::from_le_bytes(bytes);
        }
        if player_ok {
            tracing::info!(
                "[PHASE8_48_INOTIA1_PLAYER_OBJECT] event={event} ptr={:#010x} words={player_words:08x?}",
                INOTIA1_MAIN_EXP_ADDR
            );
        }
    }

    fn trace_inotia1_entity_reward_write(&mut self, write: Inotia1EntityRewardWrite) {
        if self.inotia1_entity_reward_events >= INOTIA1_ENTITY_REWARD_EVENT_LIMIT {
            if !self.inotia1_entity_reward_saturated {
                self.inotia1_entity_reward_saturated = true;
                tracing::info!(
                    "[PHASE8_48_INOTIA1_ENTITY_REWARD_TRACE_LIMIT] reached {} entity-base writes; further entity reward writes suppressed for this arm window",
                    INOTIA1_ENTITY_REWARD_EVENT_LIMIT
                );
            }
            return;
        }
        self.inotia1_entity_reward_events = self.inotia1_entity_reward_events.saturating_add(1);
        let event = self.inotia1_entity_reward_events;
        let pc_after = self.cpu.reg_get(Mode::User, reg::PC);
        let lr = self.cpu.reg_get(Mode::User, reg::LR);
        let sp = self.cpu.reg_get(Mode::User, reg::SP);
        let regs = [
            self.cpu.reg_get(Mode::User, 0), self.cpu.reg_get(Mode::User, 1),
            self.cpu.reg_get(Mode::User, 2), self.cpu.reg_get(Mode::User, 3),
            self.cpu.reg_get(Mode::User, 4), self.cpu.reg_get(Mode::User, 5),
            self.cpu.reg_get(Mode::User, 6), self.cpu.reg_get(Mode::User, 7),
            self.cpu.reg_get(Mode::User, 8), self.cpu.reg_get(Mode::User, 9),
            self.cpu.reg_get(Mode::User, 10), self.cpu.reg_get(Mode::User, 11),
            self.cpu.reg_get(Mode::User, 12),
        ];
        let old_signed = write.old_value as i32;
        let new_signed = write.new_value as i32;
        let old_low16 = write.old_value & 0xffff;
        let new_low16 = write.new_value & 0xffff;
        let old_s16 = old_low16 as u16 as i16;
        let new_s16 = new_low16 as u16 as i16;

        tracing::info!(
            "[PHASE8_48_INOTIA1_ENTITY_REWARD_WRITE] event={event} slot={} field_addr={:#010x} store_addr={:#010x} store_size={} old={:#010x}/{} new={:#010x}/{} old_low16={:#06x}/{} new_low16={:#06x}/{} pc_before={:#010x} pc_after={:#010x} lr={:#010x} sp={:#010x} r0={:#010x} r1={:#010x} r2={:#010x} r3={:#010x} r4={:#010x} r5={:#010x} r6={:#010x} r7={:#010x} r8={:#010x} r9={:#010x} r10={:#010x} r11={:#010x} r12={:#010x}",
            write.slot, write.field_addr, write.store_addr, write.store_size_bytes,
            write.old_value, old_signed, write.new_value, new_signed,
            old_low16, old_s16, new_low16, new_s16,
            write.pc_before, pc_after, lr, sp,
            regs[0], regs[1], regs[2], regs[3], regs[4], regs[5], regs[6], regs[7],
            regs[8], regs[9], regs[10], regs[11], regs[12],
        );

        let code_base = write.pc_before.saturating_sub(32) & !1;
        let mut code = [0u8; 80];
        if self.mem.read_range(code_base, code.len(), &mut code).is_ok() {
            tracing::info!(
                "[PHASE8_48_INOTIA1_ENTITY_REWARD_CODE] event={event} base={code_base:#010x} bytes={code:02x?}"
            );
        }

        let stack_base = sp.saturating_sub(32) & !3;
        let mut stack = [0u32; 24];
        let mut stack_ok = true;
        for (index, word) in stack.iter_mut().enumerate() {
            let address = stack_base.wrapping_add(index as u32 * 4);
            let mut bytes = [0u8; 4];
            if self.mem.read_range(address, 4, &mut bytes).is_err() {
                stack_ok = false;
                break;
            }
            *word = u32::from_le_bytes(bytes);
        }
        if stack_ok {
            tracing::info!(
                "[PHASE8_48_INOTIA1_ENTITY_REWARD_STACK] event={event} base={stack_base:#010x} words={stack:08x?}"
            );
        }

        let mut entity_words = [0u32; 16];
        let mut entity_ok = true;
        for (index, word) in entity_words.iter_mut().enumerate() {
            let address = write.field_addr.wrapping_add(index as u32 * 4);
            let mut bytes = [0u8; 4];
            if self.mem.read_range(address, 4, &mut bytes).is_err() {
                entity_ok = false;
                break;
            }
            *word = u32::from_le_bytes(bytes);
        }
        if entity_ok {
            tracing::info!(
                "[PHASE8_48_INOTIA1_ENTITY_REWARD_OBJECT] event={event} slot={} ptr={:#010x} words={entity_words:08x?}",
                write.slot, write.field_addr
            );
        }

        // Make signed-width provenance obvious. If the bad value arrived via
        // sign-extension from a 16-bit resource field, the exact 32-bit value
        // or its low 16 bits should usually still be visible in a source register.
        tracing::info!(
            "[PHASE8_48_INOTIA1_ENTITY_REWARD_SOURCE] event={event} new={:#010x}/{} low16={:#06x}/{} r0={:#010x} r1={:#010x} r2={:#010x} r3={:#010x} r4={:#010x} r5={:#010x} r6={:#010x} r7={:#010x} r8={:#010x} r9={:#010x} r10={:#010x} r11={:#010x} r12={:#010x}",
            write.new_value, new_signed, new_low16, new_s16,
            regs[0], regs[1], regs[2], regs[3], regs[4], regs[5], regs[6], regs[7],
            regs[8], regs[9], regs[10], regs[11], regs[12],
        );
    }

    #[inline(always)]
    fn sign_extend_12(value: u32) -> i32 {
        let value = (value & 0x0fff) as i32;
        if value & 0x0800 != 0 { value - 0x1000 } else { value }
    }

    fn trace_inotia1_exp_candidate(&mut self, candidate: Inotia1ExpCandidate) {
        // Once saturated, return before even touching the repeat-history table;
        // the temporary diagnostic must not keep accumulating bookkeeping while
        // normal gameplay continues after the capture window.
        if self.inotia1_exp_diag_events >= INOTIA1_EXP_DIAG_EVENT_LIMIT {
            if !self.inotia1_exp_diag_saturated {
                self.inotia1_exp_diag_saturated = true;
                tracing::info!(
                    "[PHASE8_48_INOTIA1_EXP_TRACE_LIMIT] reached {} candidate writes; further EXP-candidate logging suppressed for this session",
                    INOTIA1_EXP_DIAG_EVENT_LIMIT
                );
            }
            return;
        }

        // Phase 8.46 field-driven noise guards. Phase 8.45 proved that manual
        // arming works, but one 16-bit RGB565/pixel writer at 0x001069c2 touched
        // hundreds of distinct heap addresses and consumed 596/600 events in
        // ~76 ms. Suppress that exact observed writer first. Then cap any other
        // single native callsite+width across addresses so a different bulk
        // copy/render loop cannot monopolize the trace. The real EXP routine
        // can still be observed repeatedly across many kills (24 events/callsite).
        if candidate.width_bits == 16 && candidate.pc_before == INOTIA1_EXP_DIAG_NOISY_RGB565_PC {
            return;
        }

        // Keep up to four observations for each exact address+callsite+width so
        // repeated EXP changes remain comparable while hot state counters do
        // not consume the broader per-callsite allowance.
        if let Some(seen) = self.inotia1_exp_diag_seen.iter_mut().find(|seen| {
            seen.addr == candidate.addr
                && seen.pc_before == candidate.pc_before
                && seen.width_bits == candidate.width_bits
        }) {
            if seen.count >= INOTIA1_EXP_DIAG_REPEAT_LIMIT {
                return;
            }
            seen.count += 1;
        } else {
            self.inotia1_exp_diag_seen.push(Inotia1ExpSeenWrite {
                addr: candidate.addr,
                pc_before: candidate.pc_before,
                width_bits: candidate.width_bits,
                count: 1,
            });
        }

        // Count only candidates that survived the exact-address repeat guard.
        // This preserves room for the same real routine to touch several
        // related player/party fields without one hot address consuming all 24.
        if let Some(seen) = self.inotia1_exp_diag_callsites.iter_mut().find(|seen| {
            seen.pc_before == candidate.pc_before && seen.width_bits == candidate.width_bits
        }) {
            if seen.count >= INOTIA1_EXP_DIAG_CALLSITE_LIMIT {
                return;
            }
            seen.count += 1;
        } else {
            self.inotia1_exp_diag_callsites.push(Inotia1ExpSeenCallsite {
                pc_before: candidate.pc_before,
                width_bits: candidate.width_bits,
                count: 1,
            });
        }
        self.inotia1_exp_diag_events += 1;
        let event = self.inotia1_exp_diag_events;

        let pc_after = self.cpu.reg_get(Mode::User, reg::PC);
        let lr = self.cpu.reg_get(Mode::User, reg::LR);
        let sp = self.cpu.reg_get(Mode::User, reg::SP);
        let r0 = self.cpu.reg_get(Mode::User, 0);
        let r1 = self.cpu.reg_get(Mode::User, 1);
        let r2 = self.cpu.reg_get(Mode::User, 2);
        let r3 = self.cpu.reg_get(Mode::User, 3);
        let r4 = self.cpu.reg_get(Mode::User, 4);
        let r5 = self.cpu.reg_get(Mode::User, 5);
        let r6 = self.cpu.reg_get(Mode::User, 6);
        let r7 = self.cpu.reg_get(Mode::User, 7);
        let r8 = self.cpu.reg_get(Mode::User, 8);
        let r9 = self.cpu.reg_get(Mode::User, 9);
        let r10 = self.cpu.reg_get(Mode::User, 10);
        let r11 = self.cpu.reg_get(Mode::User, 11);
        let r12 = self.cpu.reg_get(Mode::User, 12);
        let delta = candidate.new as i64 - candidate.old as i64;

        let mut around = [0u32; 12];
        let around_base = candidate.addr.saturating_sub(20) & !3;
        let mut around_ok = true;
        for (index, word) in around.iter_mut().enumerate() {
            let address = around_base.wrapping_add(index as u32 * 4);
            let mut bytes = [0u8; 4];
            if self.mem.read_range(address, 4, &mut bytes).is_err() {
                around_ok = false;
                break;
            }
            *word = u32::from_le_bytes(bytes);
        }

        let code_base = candidate.pc_before.saturating_sub(8) & !1;
        let mut code = [0u8; 20];
        let code_ok = self.mem.read_range(code_base, code.len(), &mut code).is_ok();

        tracing::info!(
            "[PHASE8_48_INOTIA1_EXP_CANDIDATE] event={event} width={} addr={:#010x} old={} new={} delta={:+} old_hex={:#010x} new_hex={:#010x} pc_before={:#010x} pc_after={:#010x} lr={:#010x} sp={:#010x} r0={:#010x} r1={:#010x} r2={:#010x} r3={:#010x} r4={:#010x} r5={:#010x} r6={:#010x} r7={:#010x} r8={:#010x} r9={:#010x} r10={:#010x} r11={:#010x} r12={:#010x}",
            candidate.width_bits,
            candidate.addr,
            candidate.old,
            candidate.new,
            delta,
            candidate.old,
            candidate.new,
            candidate.pc_before,
            pc_after,
            lr,
            sp,
            r0,
            r1,
            r2,
            r3,
            r4,
            r5,
            r6,
            r7,
            r8,
            r9,
            r10,
            r11,
            r12,
        );

        if around_ok {
            tracing::info!(
                "[PHASE8_48_INOTIA1_EXP_AROUND] event={event} base={around_base:#010x} words=[{:#010x},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x}]",
                around[0], around[1], around[2], around[3], around[4], around[5], around[6], around[7], around[8], around[9], around[10], around[11]
            );
        }
        if code_ok {
            tracing::info!(
                "[PHASE8_48_INOTIA1_EXP_CODE] event={event} base={code_base:#010x} bytes={code:02x?}"
            );
        }

        // At the actual EXP store, one of the live registers often still
        // carries the player or defeated-monster object pointer. Emit compact
        // object heads for plausible guest pointers so repeated monster-family
        // identifiers can be compared without tracing every memory access.
        let regs = [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12];
        let mut emitted = 0u32;
        for (index, ptr) in regs.into_iter().enumerate() {
            if emitted >= 4 || !Self::is_inotia1_diag_data_address(ptr) || ptr & 3 != 0 {
                continue;
            }
            let mut words = [0u32; 12];
            let mut ok = true;
            for (word_index, word) in words.iter_mut().enumerate() {
                let address = ptr.wrapping_add(word_index as u32 * 4);
                let mut bytes = [0u8; 4];
                if self.mem.read_range(address, 4, &mut bytes).is_err() {
                    ok = false;
                    break;
                }
                *word = u32::from_le_bytes(bytes);
            }
            if ok {
                tracing::info!(
                    "[PHASE8_48_INOTIA1_OBJECT_HEAD] event={event} reg=r{index} ptr={ptr:#010x} words=[{:#010x},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x}]",
                    words[0], words[1], words[2], words[3], words[4], words[5], words[6], words[7], words[8], words[9], words[10], words[11]
                );
                emitted += 1;
            }
        }
    }

    #[inline(always)]
    fn inotia1_write_overlaps_exp(addr: u32, size_bytes: u32) -> bool {
        let start = addr as u64;
        let end = start.saturating_add(size_bytes as u64);
        let exp_start = INOTIA1_MAIN_EXP_ADDR as u64;
        let exp_end = exp_start + 4;
        start < exp_end && end > exp_start
    }

    #[inline(always)]
    fn inotia1_entity_reward_slot_for_write(addr: u32, size_bytes: u32) -> Option<(u32, u32)> {
        if size_bytes == 0 {
            return None;
        }
        let start = addr as u64;
        let end = start.saturating_add(size_bytes as u64);
        let first_field = INOTIA1_ENTITY_BASE_ADDR.wrapping_add(INOTIA1_ENTITY_STRIDE);
        let last_field = INOTIA1_ENTITY_BASE_ADDR
            .wrapping_add((INOTIA1_ENTITY_SLOT_COUNT - 1).wrapping_mul(INOTIA1_ENTITY_STRIDE));
        if end <= first_field as u64 || start >= last_field as u64 + 4 {
            return None;
        }

        // Derive the nearest slot arithmetically instead of scanning all 31
        // slots on every guest store while the diagnostic is armed. Check the
        // nearest slot and its neighbors so unaligned writes beginning just
        // before a four-byte field are still detected.
        let rel = addr.saturating_sub(INOTIA1_ENTITY_BASE_ADDR);
        let approx = (rel / INOTIA1_ENTITY_STRIDE).clamp(1, INOTIA1_ENTITY_SLOT_COUNT - 1);
        let first_slot = approx.saturating_sub(1).max(1);
        let last_slot = approx.saturating_add(1).min(INOTIA1_ENTITY_SLOT_COUNT - 1);
        for slot in first_slot..=last_slot {
            let field_addr = INOTIA1_ENTITY_BASE_ADDR.wrapping_add(slot.wrapping_mul(INOTIA1_ENTITY_STRIDE));
            let field_start = field_addr as u64;
            if start < field_start + 4 && end > field_start {
                return Some((slot, field_addr));
            }
        }
        None
    }

    #[inline(always)]
    fn is_inotia1_diag_data_address(addr: u32) -> bool {
        // Include the native image/BSS/runtime-data window used by the KTF title
        // plus WIE's global allocator. Unmapped addresses are never observed by the instrumented stores.
        (0x0010_0000..0x0100_0000).contains(&addr) || (0x4000_0000..0x5000_0000).contains(&addr)
    }

    fn read_svc_result(&mut self) -> Result<EngineRunResult> {
        let lr = self.cpu.reg_get(Mode::Supervisor, reg::LR);
        let spsr = self.cpu.reg_get(Mode::Supervisor, reg::SPSR);

        let svc_address = lr.checked_sub(2).ok_or(WieError::InvalidMemoryAccess(lr))?;
        let mut svc_bytes = [0u8; 2];
        self.mem.read_range(svc_address, 2, &mut svc_bytes)?;
        let instruction = u16::from_le_bytes(svc_bytes);
        if instruction & 0xff00 != 0xdf00 {
            return Err(WieError::FatalError(format!(
                "Invalid Thumb SVC instruction {instruction:#06x} at {svc_address:#x}"
            )));
        }

        let category = instruction as u32 & 0xff;

        Ok(EngineRunResult::Svc { category, lr, spsr })
    }
}

impl ArmEngine for Arm32CpuEngine {
    fn run(&mut self, end: u32, mut count: u32) -> Result<EngineRunResult> {
        loop {
            // Phase 8.24 — interpreter dispatch hot path. The old loop read PC
            // once here and then again inside is_svc_exception() for *every*
            // guest instruction. Inotia 2's graphics loops execute millions of
            // Thumb instructions between presents, so that redundant register
            // lookup is measurable interpreter overhead. Reuse the already-read
            // PC and only touch CPSR on the exceptional vector address itself.
            let pc = self.cpu.reg_get(Mode::User, reg::PC);

            if pc == 0x08 && (self.cpu.reg_get(Mode::User, reg::CPSR) & 0x1f) == 0x13 {
                return self.read_svc_result();
            }

            if pc < 0x1000 {
                // Phase 8.39 — exception-only context capture.  A field test of
                // 축복받은 부활주문서 faults at address 0x254, but the old error
                // discarded the guest instruction/call context needed to tell a
                // bad PC from a bad data pointer.  This branch runs only on an
                // already-fatal condition and therefore adds no gameplay-path
                // logging or measurable interpreter overhead.
                let lr = self.cpu.reg_get(Mode::User, reg::LR);
                let sp = self.cpu.reg_get(Mode::User, reg::SP);
                let r0 = self.cpu.reg_get(Mode::User, 0);
                let r1 = self.cpu.reg_get(Mode::User, 1);
                let r2 = self.cpu.reg_get(Mode::User, 2);
                let r3 = self.cpu.reg_get(Mode::User, 3);
                tracing::error!(
                    "[PHASE8_39_ARM_FAULT_CONTEXT] kind=pc-low fault={pc:#010x} pc={pc:#010x} lr={lr:#010x} sp={sp:#010x} r0={r0:#010x} r1={r1:#010x} r2={r2:#010x} r3={r3:#010x}"
                );
                return Err(WieError::InvalidMemoryAccess(pc));
            }

            if pc == end {
                return Ok(EngineRunResult::End);
            }

            if count == 0 {
                return Ok(EngineRunResult::CountExhausted);
            }

            let (step_ok, memory_error, exact_exp_write, entity_reward_write, exp_candidate) = {
                let mut arm32cpu_memory = self
                    .mem
                    .as_arm32cpu_memory(self.inotia1_exp_diag_enabled, pc);
                let step_ok = self.cpu.step(&mut arm32cpu_memory);
                (
                    step_ok,
                    arm32cpu_memory.memory_error(),
                    arm32cpu_memory.inotia1_exact_exp_write(),
                    arm32cpu_memory.inotia1_entity_reward_write(),
                    arm32cpu_memory.inotia1_exp_candidate(),
                )
            };

            if !step_ok {
                return Err(WieError::FatalError("Undefined instruction".into()));
            }
            count -= 1;

            if let Some(write) = exact_exp_write {
                // Exact EXP writes are intentionally independent from the broad
                // diagnostic's event/callsite caps. Even if the generic trace
                // saturates, every write touching 0x00171040 remains visible.
                self.trace_inotia1_exact_exp_write(write);
            }
            if let Some(write) = entity_reward_write {
                self.trace_inotia1_entity_reward_write(write);
            }
            if let Some(candidate) = exp_candidate {
                self.trace_inotia1_exp_candidate(candidate);
            }

            if let Some(x) = memory_error {
                // Phase 8.39 — same exception-only trace for a data-memory
                // fault.  Capturing PC/LR here lets the next blessed-revival
                // test identify the exact native instruction without any
                // high-frequency probes.
                let fault_pc = self.cpu.reg_get(Mode::User, reg::PC);
                let lr = self.cpu.reg_get(Mode::User, reg::LR);
                let sp = self.cpu.reg_get(Mode::User, reg::SP);
                let r0 = self.cpu.reg_get(Mode::User, 0);
                let r1 = self.cpu.reg_get(Mode::User, 1);
                let r2 = self.cpu.reg_get(Mode::User, 2);
                let r3 = self.cpu.reg_get(Mode::User, 3);
                tracing::error!(
                    "[PHASE8_39_ARM_FAULT_CONTEXT] kind=data-memory fault={x:#010x} pc={fault_pc:#010x} lr={lr:#010x} sp={sp:#010x} r0={r0:#010x} r1={r1:#010x} r2={r2:#010x} r3={r3:#010x}"
                );
                return Err(WieError::InvalidMemoryAccess(x));
            }
        }
    }

    fn reg_write(&mut self, reg: ArmRegister, value: u32) {
        if reg == ArmRegister::PC && value % 2 == 1 {
            self.cpu.reg_set(Mode::User, reg.into_armv4t(), value - 1);

            let cpsr = self.cpu.reg_get(Mode::User, reg::CPSR);
            self.cpu.reg_set(Mode::User, reg::CPSR, cpsr | (1 << 5)); // T bit

            return;
        }
        self.cpu.reg_set(Mode::User, reg.into_armv4t(), value);
    }

    fn reg_read(&self, reg: ArmRegister) -> u32 {
        self.cpu.reg_get(Mode::User, reg.into_armv4t())
    }

    fn mem_map(&mut self, address: u32, size: usize, _permission: MemoryPermission) {
        self.mem.map(address, size);
    }

    fn mem_write(&mut self, address: u32, data: &[u8]) -> Result<()> {
        if !self.inotia1_exp_diag_enabled {
            return self.mem.write_range(address, data);
        }

        let overlaps_exp = Self::inotia1_write_overlaps_exp(address, data.len() as u32);
        let entity_slot = Self::inotia1_entity_reward_slot_for_write(address, data.len() as u32);
        if !overlaps_exp && entity_slot.is_none() {
            return self.mem.write_range(address, data);
        }

        let mut exp_before = [0u8; 4];
        let exp_before_ok = overlaps_exp && self.mem.read_range(INOTIA1_MAIN_EXP_ADDR, 4, &mut exp_before).is_ok();
        let mut entity_before = [0u8; 4];
        let entity_before_ok = if let Some((_, field_addr)) = entity_slot {
            self.mem.read_range(field_addr, 4, &mut entity_before).is_ok()
        } else {
            false
        };

        self.mem.write_range(address, data)?;

        if exp_before_ok {
            let mut after = [0u8; 4];
            if self.mem.read_range(INOTIA1_MAIN_EXP_ADDR, 4, &mut after).is_ok() {
                let old_exp = u32::from_le_bytes(exp_before);
                let new_exp = u32::from_le_bytes(after);
                if old_exp != new_exp {
                    self.trace_inotia1_exact_exp_write(Inotia1ExactExpWrite {
                        store_addr: address,
                        store_size_bytes: data.len() as u32,
                        old_exp,
                        new_exp,
                        pc_before: self.cpu.reg_get(Mode::User, reg::PC),
                    });
                }
            }
        }

        if entity_before_ok {
            if let Some((slot, field_addr)) = entity_slot {
                let mut after = [0u8; 4];
                if self.mem.read_range(field_addr, 4, &mut after).is_ok() {
                    let old_value = u32::from_le_bytes(entity_before);
                    let new_value = u32::from_le_bytes(after);
                    if old_value != new_value {
                        self.trace_inotia1_entity_reward_write(Inotia1EntityRewardWrite {
                            slot,
                            field_addr,
                            store_addr: address,
                            store_size_bytes: data.len() as u32,
                            old_value,
                            new_value,
                            pc_before: self.cpu.reg_get(Mode::User, reg::PC),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn mem_read(&mut self, address: u32, size: usize, result: &mut [u8]) -> Result<usize> {
        self.mem.read_range(address, size, result)
    }

    fn is_mapped(&self, address: u32, size: usize) -> bool {
        self.mem.is_mapped(address, size)
    }

    fn set_inotia1_exp_diagnostics(&mut self, enabled: bool) {
        self.inotia1_exp_diag_enabled = enabled;
        self.inotia1_exp_diag_events = 0;
        self.inotia1_exp_diag_saturated = false;
        self.inotia1_exp_diag_seen.clear();
        self.inotia1_exp_diag_callsites.clear();
        self.inotia1_exact_exp_events = 0;
        self.inotia1_entity_reward_events = 0;
        self.inotia1_entity_reward_saturated = false;
        if enabled {
            let mut bytes = [0u8; 4];
            if self.mem.read_range(INOTIA1_MAIN_EXP_ADDR, 4, &mut bytes).is_ok() {
                let value = u32::from_le_bytes(bytes);
                tracing::info!(
                    "[PHASE8_48_INOTIA1_EXP_BASELINE] addr={:#010x} value={} value_hex={:#010x}",
                    INOTIA1_MAIN_EXP_ADDR,
                    value,
                    value
                );
            }
            for slot in 1..INOTIA1_ENTITY_SLOT_COUNT {
                let field_addr = INOTIA1_ENTITY_BASE_ADDR.wrapping_add(slot.wrapping_mul(INOTIA1_ENTITY_STRIDE));
                let mut field = [0u8; 4];
                if self.mem.read_range(field_addr, 4, &mut field).is_ok() {
                    let raw = u32::from_le_bytes(field);
                    if raw != 0 {
                        let low16 = raw & 0xffff;
                        tracing::info!(
                            "[PHASE8_48_INOTIA1_ENTITY_REWARD_BASELINE] slot={} addr={:#010x} raw={:#010x}/{} low16={:#06x}/{}",
                            slot, field_addr, raw, raw as i32, low16, low16 as u16 as i16
                        );
                    }
                }
            }
        }
    }
}

impl ArmRegister {
    fn into_armv4t(self) -> u8 {
        match self {
            ArmRegister::R0 => 0,
            ArmRegister::R1 => 1,
            ArmRegister::R2 => 2,
            ArmRegister::R3 => 3,
            ArmRegister::R4 => 4,
            ArmRegister::R5 => 5,
            ArmRegister::R6 => 6,
            ArmRegister::R7 => 7,
            ArmRegister::R8 => 8,
            ArmRegister::SB => 9,
            ArmRegister::SL => 10,
            ArmRegister::FP => 11,
            ArmRegister::IP => 12,
            ArmRegister::SP => reg::SP,
            ArmRegister::LR => reg::LR,
            ArmRegister::PC => reg::PC,
            ArmRegister::Cpsr => reg::CPSR,
        }
    }
}

const TOTAL_MEMORY: u64 = 0x100000000;
const PAGE_SIZE: usize = 0x10000;
const PAGE_MASK: u32 = (PAGE_SIZE - 1) as _;

struct EmulatedMemory {
    pages: Box<[Option<Box<[u8; PAGE_SIZE]>>]>,
}

impl EmulatedMemory {
    fn new() -> Self {
        Self {
            pages: vec![None; (TOTAL_MEMORY / PAGE_SIZE as u64) as usize].into_boxed_slice(),
        }
    }

    fn as_arm32cpu_memory(&mut self, inotia1_exp_diag_enabled: bool, pc_before: u32) -> Arm32CpuMemory<'_> {
        Arm32CpuMemory::new(self, inotia1_exp_diag_enabled, pc_before)
    }

    fn map(&mut self, address: u32, size: usize) {
        let page_start = address & !PAGE_MASK;
        let page_end = (address + size as u32 + PAGE_MASK) & !PAGE_MASK;

        for page in (page_start..page_end).step_by(PAGE_SIZE) {
            let page_data = &mut self.pages[page as usize / PAGE_SIZE];
            if page_data.is_none() {
                *page_data = Some(Box::new([0; PAGE_SIZE]));
            }
        }
    }

    fn read_range(&self, address: u32, size: usize, result: &mut [u8]) -> Result<usize> {
        let mut remaining_size = size;
        let mut current_address = address;

        while remaining_size > 0 {
            let page_address = current_address & !PAGE_MASK;
            let page_data = self.pages[page_address as usize / PAGE_SIZE]
                .as_ref()
                .ok_or(WieError::InvalidMemoryAccess(current_address))?;
            let offset = (current_address - page_address) as usize;
            let available_bytes = (PAGE_SIZE - offset).min(remaining_size);

            result[size - remaining_size..size - remaining_size + available_bytes].copy_from_slice(&page_data[offset..offset + available_bytes]);
            remaining_size -= available_bytes;
            current_address += available_bytes as u32;
        }

        Ok(size)
    }

    fn write_range(&mut self, address: u32, data: &[u8]) -> Result<()> {
        let mut current_address = address;
        let mut data_index = 0;

        while data_index < data.len() {
            let page_address = current_address & !PAGE_MASK;
            let page_data = self.pages[page_address as usize / PAGE_SIZE]
                .as_mut()
                .ok_or(WieError::InvalidMemoryAccess(current_address))?;
            let offset = (current_address - page_address) as usize;
            let available_bytes = (PAGE_SIZE - offset).min(data.len() - data_index);

            page_data[offset..offset + available_bytes].copy_from_slice(&data[data_index..data_index + available_bytes]);
            data_index += available_bytes;
            current_address += available_bytes as u32;
        }

        Ok(())
    }

    fn is_mapped(&self, address: u32, size: usize) -> bool {
        let page_start = address & !PAGE_MASK;
        let page_end = (address + size as u32 + PAGE_MASK) & !PAGE_MASK;

        if self.pages[page_start as usize / PAGE_SIZE].is_none() {
            return false;
        }

        for page in (page_start..page_end).step_by(PAGE_SIZE) {
            if self.pages[page as usize / PAGE_SIZE].is_none() {
                return false;
            }
        }

        true
    }
}

struct Arm32CpuMemory<'a> {
    emulated_memory: &'a mut EmulatedMemory,
    // [PHASE8_22_ARM_MEMORY_FASTPATH] Memory callbacks already receive &mut self, so interior
    // mutability is unnecessary. Keeping the error slot as a plain Option
    // removes RefCell borrow checks from every guest memory access.
    memory_error: Option<u32>,
    inotia1_exp_diag_enabled: bool,
    pc_before: u32,
    inotia1_exact_exp_write: Option<Inotia1ExactExpWrite>,
    inotia1_entity_reward_write: Option<Inotia1EntityRewardWrite>,
    inotia1_exp_candidate: Option<Inotia1ExpCandidate>,
}

impl<'a> Arm32CpuMemory<'a> {
    fn new(emulated_memory: &'a mut EmulatedMemory, inotia1_exp_diag_enabled: bool, pc_before: u32) -> Self {
        Self {
            emulated_memory,
            memory_error: None,
            inotia1_exp_diag_enabled,
            pc_before,
            inotia1_exact_exp_write: None,
            inotia1_entity_reward_write: None,
            inotia1_exp_candidate: None,
        }
    }

    #[inline(always)]
    fn memory_error(&self) -> Option<u32> {
        self.memory_error
    }

    #[inline(always)]
    fn inotia1_exact_exp_write(&self) -> Option<Inotia1ExactExpWrite> {
        self.inotia1_exact_exp_write
    }

    #[inline(always)]
    fn read_inotia1_exact_exp_word(&mut self) -> Option<u32> {
        let offset = (INOTIA1_MAIN_EXP_ADDR & PAGE_MASK) as usize;
        let data = self.get_page(INOTIA1_MAIN_EXP_ADDR)?;
        let raw = unsafe { core::ptr::read_unaligned(data.as_ptr().add(offset).cast::<u32>()) };
        Some(u32::from_le(raw))
    }

    #[inline(always)]
    fn capture_inotia1_exact_exp_write(&mut self, store_addr: u32, store_size_bytes: u32, old_exp: Option<u32>) {
        if !self.inotia1_exp_diag_enabled
            || self.inotia1_exact_exp_write.is_some()
            || !Arm32CpuEngine::inotia1_write_overlaps_exp(store_addr, store_size_bytes)
        {
            return;
        }
        let Some(old_exp) = old_exp else { return; };
        let Some(new_exp) = self.read_inotia1_exact_exp_word() else { return; };
        if old_exp == new_exp {
            return;
        }
        self.inotia1_exact_exp_write = Some(Inotia1ExactExpWrite {
            store_addr,
            store_size_bytes,
            old_exp,
            new_exp,
            pc_before: self.pc_before,
        });
    }

    #[inline(always)]
    fn inotia1_entity_reward_write(&self) -> Option<Inotia1EntityRewardWrite> {
        self.inotia1_entity_reward_write
    }

    #[inline(always)]
    fn read_inotia1_entity_reward_word(&mut self, field_addr: u32) -> Option<u32> {
        let offset = (field_addr & PAGE_MASK) as usize;
        let data = self.get_page(field_addr)?;
        let raw = unsafe { core::ptr::read_unaligned(data.as_ptr().add(offset).cast::<u32>()) };
        Some(u32::from_le(raw))
    }

    #[inline(always)]
    fn capture_inotia1_entity_reward_write(
        &mut self,
        store_addr: u32,
        store_size_bytes: u32,
        old_value: Option<(u32, u32, u32)>,
    ) {
        if !self.inotia1_exp_diag_enabled || self.inotia1_entity_reward_write.is_some() {
            return;
        }
        let Some((slot, field_addr, old_value)) = old_value else { return; };
        let Some(new_value) = self.read_inotia1_entity_reward_word(field_addr) else { return; };
        if old_value == new_value {
            return;
        }
        self.inotia1_entity_reward_write = Some(Inotia1EntityRewardWrite {
            slot,
            field_addr,
            store_addr,
            store_size_bytes,
            old_value,
            new_value,
            pc_before: self.pc_before,
        });
    }

    #[inline(always)]
    fn read_inotia1_entity_reward_before(&mut self, store_addr: u32, store_size_bytes: u32) -> Option<(u32, u32, u32)> {
        if !self.inotia1_exp_diag_enabled {
            return None;
        }
        let (slot, field_addr) = Arm32CpuEngine::inotia1_entity_reward_slot_for_write(store_addr, store_size_bytes)?;
        let old_value = self.read_inotia1_entity_reward_word(field_addr)?;
        Some((slot, field_addr, old_value))
    }

    #[inline(always)]
    fn inotia1_exp_candidate(&self) -> Option<Inotia1ExpCandidate> {
        self.inotia1_exp_candidate
    }

    #[inline(always)]
    fn consider_inotia1_exp_candidate(&mut self, addr: u32, old: u32, new: u32, width_bits: u8) {
        if !self.inotia1_exp_diag_enabled || self.inotia1_exp_candidate.is_some() || old == new {
            return;
        }
        if !Arm32CpuEngine::is_inotia1_diag_data_address(addr) {
            return;
        }
        // Inotia1's client.bin executes in the low 0x001xxxxx image window.
        // Reject runtime/helper writes so the event budget stays focused on
        // native game-code stores.
        if !(0x0010_0000..0x0020_0000).contains(&self.pc_before) {
            return;
        }

        // Phase 8.46 retains the 8.44 widened probe to both STRH (16-bit) and STR
        // (32-bit) stores and intentionally removes the old >=4096 signal
        // floor. The first field log proved that floor/width combination could
        // miss the real EXP update entirely. Keep startup/render noise bounded
        // with structural filters instead of assuming EXP must be large.
        if old == 0 || new == 0 {
            return;
        }
        let delta = old.abs_diff(new);
        if delta <= 1 {
            return;
        }

        // Ignore tiny state/animation counters while retaining observed EXP
        // changes such as ~150 and negative changes of similar magnitude.
        if old <= 31 && new <= 31 {
            return;
        }

        // The field log shows the guest stack around 0x400ffxxx. Stack-local
        // temporaries change too frequently to be useful as persistent EXP.
        if (0x400f_0000..0x4010_0000).contains(&addr) {
            return;
        }

        match width_bits {
            16 => {
                if delta > 30_000 {
                    return;
                }
            }
            32 => {
                const MAX_VALUE: u32 = 50_000_000;
                const MAX_ABS_DELTA: u32 = 250_000;
                if old > MAX_VALUE || new > MAX_VALUE || delta > MAX_ABS_DELTA {
                    return;
                }

                // Pointer-to-pointer bookkeeping can look like a numeric delta
                // in the 32-bit watcher. Skip it when both values are aligned
                // plausible guest pointers; actual EXP values remain eligible.
                if old & 3 == 0
                    && new & 3 == 0
                    && Arm32CpuEngine::is_inotia1_diag_data_address(old)
                    && Arm32CpuEngine::is_inotia1_diag_data_address(new)
                {
                    return;
                }
            }
            _ => return,
        }

        self.inotia1_exp_candidate = Some(Inotia1ExpCandidate {
            addr,
            old,
            new,
            width_bits,
            pc_before: self.pc_before,
        });
    }

    #[inline(always)]
    fn get_page(&mut self, addr: u32) -> Option<&mut [u8; PAGE_SIZE]> {
        // `addr` is u32 and PAGE_SIZE is 64 KiB, so this index is always in
        // 0..65536, exactly matching the fixed page table. Avoid a redundant
        // bounds check in the hottest interpreter path.
        let page_index = (addr >> 16) as usize;
        let page_data = unsafe { self.emulated_memory.pages.get_unchecked_mut(page_index) }.as_mut();

        if let Some(x) = page_data {
            Some(x)
        } else {
            self.memory_error = Some(addr);
            None
        }
    }
}

impl Memory for Arm32CpuMemory<'_> {
    #[inline(always)]
    fn r8(&mut self, addr: u32) -> u8 {
        let offset = (addr & PAGE_MASK) as usize;
        match self.get_page(addr) {
            Some(data) => data[offset],
            None => 0,
        }
    }

    #[inline(always)]
    fn r16(&mut self, addr: u32) -> u16 {
        let offset = (addr & PAGE_MASK) as usize;
        if offset <= PAGE_SIZE - 2 {
            let Some(data) = self.get_page(addr) else { return 0; };
            // Guest memory is little-endian; unaligned halfword access is
            // permitted by the emulator and maps efficiently to WASM loads.
            let raw = unsafe { core::ptr::read_unaligned(data.as_ptr().add(offset).cast::<u16>()) };
            return u16::from_le(raw);
        }

        // Rare page-crossing access: preserve exact old semantics without
        // indexing past the 64 KiB page.
        let b0 = self.r8(addr) as u16;
        let b1 = self.r8(addr.wrapping_add(1)) as u16;
        b0 | (b1 << 8)
    }

    #[inline(always)]
    fn r32(&mut self, addr: u32) -> u32 {
        let offset = (addr & PAGE_MASK) as usize;
        if offset <= PAGE_SIZE - 4 {
            let Some(data) = self.get_page(addr) else { return 0; };
            let raw = unsafe { core::ptr::read_unaligned(data.as_ptr().add(offset).cast::<u32>()) };
            return u32::from_le(raw);
        }

        let b0 = self.r8(addr) as u32;
        let b1 = self.r8(addr.wrapping_add(1)) as u32;
        let b2 = self.r8(addr.wrapping_add(2)) as u32;
        let b3 = self.r8(addr.wrapping_add(3)) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    #[inline(always)]
    fn w8(&mut self, addr: u32, val: u8) {
        let exact_old = if self.inotia1_exp_diag_enabled && Arm32CpuEngine::inotia1_write_overlaps_exp(addr, 1) {
            self.read_inotia1_exact_exp_word()
        } else {
            None
        };
        let entity_old = self.read_inotia1_entity_reward_before(addr, 1);
        let offset = (addr & PAGE_MASK) as usize;
        if let Some(data) = self.get_page(addr) {
            data[offset] = val;
        }
        self.capture_inotia1_exact_exp_write(addr, 1, exact_old);
        self.capture_inotia1_entity_reward_write(addr, 1, entity_old);
    }

    #[inline(always)]
    fn w16(&mut self, addr: u32, val: u16) {
        let exact_old = if self.inotia1_exp_diag_enabled && Arm32CpuEngine::inotia1_write_overlaps_exp(addr, 2) {
            self.read_inotia1_exact_exp_word()
        } else {
            None
        };
        let entity_old = self.read_inotia1_entity_reward_before(addr, 2);
        let offset = (addr & PAGE_MASK) as usize;
        if offset <= PAGE_SIZE - 2 {
            if !self.inotia1_exp_diag_enabled {
                let Some(data) = self.get_page(addr) else { return; };
                unsafe {
                    core::ptr::write_unaligned(data.as_mut_ptr().add(offset).cast::<u16>(), val.to_le());
                }
                return;
            }

            let old = {
                let Some(data) = self.get_page(addr) else { return; };
                let raw = unsafe { core::ptr::read_unaligned(data.as_ptr().add(offset).cast::<u16>()) };
                let old = u16::from_le(raw);
                unsafe {
                    core::ptr::write_unaligned(data.as_mut_ptr().add(offset).cast::<u16>(), val.to_le());
                }
                old
            };
            self.capture_inotia1_exact_exp_write(addr, 2, exact_old);
            self.capture_inotia1_entity_reward_write(addr, 2, entity_old);
            self.consider_inotia1_exp_candidate(addr, old as u32, val as u32, 16);
            return;
        }

        // Rare page-crossing halfword store. Capture the old value before the
        // byte-wise fallback so the widened diagnostic still sees STRH here.
        let old = if self.inotia1_exp_diag_enabled { Some(self.r16(addr)) } else { None };
        self.w8(addr, val as u8);
        self.w8(addr.wrapping_add(1), (val >> 8) as u8);
        if let Some(old) = old {
            self.consider_inotia1_exp_candidate(addr, old as u32, val as u32, 16);
        }
    }

    #[inline(always)]
    fn w32(&mut self, addr: u32, val: u32) {
        let exact_old = if self.inotia1_exp_diag_enabled && Arm32CpuEngine::inotia1_write_overlaps_exp(addr, 4) {
            self.read_inotia1_exact_exp_word()
        } else {
            None
        };
        let entity_old = self.read_inotia1_entity_reward_before(addr, 4);
        let offset = (addr & PAGE_MASK) as usize;
        if offset <= PAGE_SIZE - 4 {
            if !self.inotia1_exp_diag_enabled {
                let Some(data) = self.get_page(addr) else { return; };
                unsafe {
                    core::ptr::write_unaligned(data.as_mut_ptr().add(offset).cast::<u32>(), val.to_le());
                }
                return;
            }

            let old = {
                let Some(data) = self.get_page(addr) else { return; };
                let raw = unsafe { core::ptr::read_unaligned(data.as_ptr().add(offset).cast::<u32>()) };
                let old = u32::from_le(raw);
                unsafe {
                    core::ptr::write_unaligned(data.as_mut_ptr().add(offset).cast::<u32>(), val.to_le());
                }
                old
            };
            self.capture_inotia1_exact_exp_write(addr, 4, exact_old);
            self.capture_inotia1_entity_reward_write(addr, 4, entity_old);
            self.consider_inotia1_exp_candidate(addr, old, val, 32);
            return;
        }

        // Rare page-crossing word store; preserve visibility in the diagnostic
        // instead of silently falling through to four uninstrumented byte writes.
        let old = if self.inotia1_exp_diag_enabled { Some(self.r32(addr)) } else { None };
        self.w8(addr, val as u8);
        self.w8(addr.wrapping_add(1), (val >> 8) as u8);
        self.w8(addr.wrapping_add(2), (val >> 16) as u8);
        self.w8(addr.wrapping_add(3), (val >> 24) as u8);
        if let Some(old) = old {
            self.consider_inotia1_exp_candidate(addr, old, val, 32);
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use core::mem::size_of;

    use arm32_cpu::Memory;

    use super::EmulatedMemory;

    #[test]
    fn page_table_is_heap_allocated() {
        assert_eq!(size_of::<EmulatedMemory>(), size_of::<Box<[Option<Box<[u8; super::PAGE_SIZE]>>]>>());
    }

    #[test]
    fn test_memory_basic() {
        let mut memory = EmulatedMemory::new();

        memory.map(0x10000, 0x1000);
        memory.map(0x11000, 0x1000);
        memory.map(0x20000, 0x10000);

        memory.write_range(0x10000, &[123; 0x1000]).unwrap();

        let mut buf = [0; 0x1000];
        memory.read_range(0x10000, 0x1000, &mut buf).unwrap();
        assert_eq!(buf, [123; 0x1000]);

        memory.write_range(0x10900, &[100; 0x1000]).unwrap();

        memory.read_range(0x10900, 0x1000, &mut buf).unwrap();
        assert_eq!(buf, [100; 0x1000]);

        let mut arm32cpu_memory = memory.as_arm32cpu_memory(false, 0);

        let r8 = arm32cpu_memory.r8(0x10000);
        assert_eq!(r8, 123);

        let r16 = arm32cpu_memory.r16(0x10000);
        assert_eq!(r16, 123 | (123 << 8));

        let r32 = arm32cpu_memory.r32(0x10000);
        assert_eq!(r32, 123 | (123 << 8) | (123 << 16) | (123 << 24));

        arm32cpu_memory.w8(0x10000, 12);
        let r8 = arm32cpu_memory.r8(0x10000);
        assert_eq!(r8, 12);

        arm32cpu_memory.w16(0x10000, 0x1234);
        let r16 = arm32cpu_memory.r16(0x10000);
        assert_eq!(r16, 0x1234);

        arm32cpu_memory.w32(0x10000, 0x12345678);
        let r32 = arm32cpu_memory.r32(0x10000);
        assert_eq!(r32, 0x12345678);
    }

    #[test]
    fn test_memory_cross_page_word_access() {
        let mut memory = EmulatedMemory::new();
        memory.map(0x10000, 0x20000);

        let mut arm32cpu_memory = memory.as_arm32cpu_memory(false, 0);
        arm32cpu_memory.w16(0x1ffff, 0x1234);
        assert_eq!(arm32cpu_memory.r16(0x1ffff), 0x1234);

        arm32cpu_memory.w32(0x1fffe, 0x89abcdef);
        assert_eq!(arm32cpu_memory.r32(0x1fffe), 0x89abcdef);
    }

    #[test]
    fn test_memory_unmapped_read() {
        let mut memory = EmulatedMemory::new();

        memory.map(0x10000, 0x10000);

        let mut buf = [0; 0x1000];
        assert!(memory.read_range(0x1f500, 0x1000, &mut buf).is_err());
    }

    #[test]
    fn test_memory_unmapped_write() {
        let mut memory = EmulatedMemory::new();

        memory.map(0x10000, 0x10000);

        assert!(memory.write_range(0x1f500, &[12; 0x1000]).is_err());
    }
}
