use alloc::{collections::BTreeMap, format, string::ToString, sync::Arc, vec::Vec};

use wie_util::{ByteRead, ByteWrite, Result, WieError, read_generic};

use super::{Entry, PatternToken, scan_pattern};
use crate::{ArmCore, engine::ArmRegister, function::JumpTo, stdlib};

const BINARY_PATCH_SVC: u32 = 0x80;

#[derive(Debug, Clone, Copy)]
pub struct Hook {
    /// LSB=1 (Thumb) is required; the engine doesn't service ARM-mode SVCs.
    pub pc: u32,
    pub kind: HookKind,
}

#[derive(Debug, Clone, Copy)]
pub enum HookKind {
    /// ABI: dst=r0, src=r1, len=r2; returns via LR.
    Memcpy,
    /// ABI: dst=r0, val=r1 (low byte), len=r2; returns via LR.
    Memset,
    /// ABI: dst=r0, src=r1; copies NUL-inclusive; returns via LR (R0 unchanged).
    Strcpy,
    /// ABI: str=r0; returns length in R0 via LR.
    Strlen,
    /// Replaces an inline byte-copy loop. Requires a down-counter `len` on the
    /// stack — zeroing it has to terminate the loop, so up-counter (`for i = 0;
    /// i < N`) shapes are not compatible.
    InlineCopy(InlineCopy),
    /// Replaces an inline byte-copy loop whose src/dst/count live in registers
    /// instead of on the stack. Bytes copied = `read(count) + count_offset`.
    /// After the copy, `src`/`dst` are advanced by that count and `count` is
    /// rewritten so it equals what the loop would have left there (i.e.,
    /// `original - bytes`), then the dispatcher jumps to `exit_pc`.
    RegInlineCopy(RegInlineCopy),
    /// Phase 8.40 — exact Inotia1 resurrection UI callsite repair.
    ///
    /// The original instruction at 0x00131cb2 is `LDR R0, [SP, #0x4c]`.
    /// The following internal call enters 0x0011dfb8, which immediately
    /// dereferences R4+0x248/R4+0x24c. Field fault capture proves this rare
    /// resurrection path reaches it with R4=4, while R8 is the live character
    /// context whose +0x248/+0x24c fields were already read by the caller.
    /// This hook emulates the replaced LDR and copies R8 into R4 before
    /// continuing to the untouched BL instruction.
    Inotia1RevivalBaseRepair,
    /// Phase 8.53 — production Inotia1 monster base-reward overflow repair using the
    /// same formula evaluated using wide intermediates. The original Thumb
    /// helper at 0x001281ec uses 32-bit MULS before signed division; higher
    /// monster parameters can wrap the numerator negative (or back positive).
    /// This hook preserves the original result unless the constructor caller
    /// (LR=0x00126245) diverges from the mathematically equivalent wide result.
    Inotia1RewardWideMathRepair,
}

#[derive(Debug, Clone, Copy)]
pub struct InlineCopy {
    pub dst_offset: i32,
    pub src_offset: i32,
    pub len_offset: i32,
    pub exit_pc: u32,
    /// Set when the loop's outer code re-reads the stack slots after the body;
    /// the dispatcher then writes back `dst+len`, `src+len`, `len=0`.
    pub spill_back: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct RegInlineCopy {
    src: ArmRegister,
    dst: ArmRegister,
    count: ArmRegister,
    count_offset: i32,
    exit_pc: u32,
}

/// Scanned across the install-time memory range; each match becomes a `Hook`.
pub struct PatternHook {
    pub tokens: Vec<PatternToken>,
    pub kind_template: PatternHookKind,
}

pub enum PatternHookKind {
    Memcpy,
    Memset,
    Strcpy,
    Strlen,
    InlineCopy {
        /// `None` => filled from the matching `{dst}` / `{src}` / `{len}`
        /// capture (Thumb1 `SUBS Rn, #imm8` byte, negated to a stack offset).
        /// `Some(v)` => pinned by TOML when the pattern omits the capture.
        dst_offset: Option<i32>,
        src_offset: Option<i32>,
        len_offset: Option<i32>,
        /// `None` => filled from the `{exit_b}` capture's decoded branch target.
        exit_pc: Option<u32>,
        spill_back: bool,
    },
    /// Pattern-matched register-resident copy loop. The `src`/`dst`/`count`
    /// registers are read from the pattern's bit-level captures and `exit_pc`
    /// is derived at install time from `match_addr + pattern.len()` (Thumb bit
    /// set).
    RegInlineCopy {
        count_offset: i32,
    },
    Inotia1RevivalBaseRepair,
}

/// Expand static + pattern hooks into a single `Vec<Hook>` whose PCs are final
/// and Thumb-valid. All pattern matching happens here; downstream consumers
/// (overlap check, `apply_hooks`) only see PC + kind, never raw tokens.
///
/// Static hook PCs come from TOML, so we validate the Thumb bit up front —
/// `install_entry` runs `apply_patches` between resolve and apply, and a
/// fatal-after-write would leave guest memory partially modified.
pub fn resolve_hooks(core: &mut ArmCore, entry: &Entry, scan_ranges: &[(u32, u32)]) -> Result<Vec<Hook>> {
    for hook in &entry.hooks {
        if hook.pc & 1 == 0 {
            return Err(WieError::FatalError(format!(
                "entry {}: hook PC {:#x} targets ARM mode; only Thumb (LSB=1) is supported",
                entry.name, hook.pc
            )));
        }
    }
    let mut installed: Vec<Hook> = entry.hooks.clone();

    for pattern in &entry.hook_patterns {
        let matches = scan_pattern(core, &pattern.tokens, scan_ranges)?;
        for (match_addr, pm) in matches {
            let kind = match &pattern.kind_template {
                PatternHookKind::Memcpy => HookKind::Memcpy,
                PatternHookKind::Memset => HookKind::Memset,
                PatternHookKind::Strcpy => HookKind::Strcpy,
                PatternHookKind::Strlen => HookKind::Strlen,
                PatternHookKind::InlineCopy {
                    dst_offset,
                    src_offset,
                    len_offset,
                    exit_pc,
                    spill_back,
                } => {
                    let dst = dst_offset
                        .or_else(|| pm.dst.map(capture_to_offset))
                        .ok_or_else(|| WieError::FatalError(format!("pattern match at {match_addr:#x} missing dst")))?;
                    let src = src_offset
                        .or_else(|| pm.src.map(capture_to_offset))
                        .ok_or_else(|| WieError::FatalError(format!("pattern match at {match_addr:#x} missing src")))?;
                    let len = len_offset
                        .or_else(|| pm.len.map(capture_to_offset))
                        .ok_or_else(|| WieError::FatalError(format!("pattern match at {match_addr:#x} missing len")))?;
                    let exit = if let Some(v) = exit_pc {
                        *v
                    } else {
                        let site = pm
                            .exit_b_site
                            .ok_or_else(|| WieError::FatalError("pattern missing exit_b site".to_string()))?;
                        let bytes = pm
                            .exit_b_bytes
                            .ok_or_else(|| WieError::FatalError("pattern missing exit_b bytes".to_string()))?;
                        decode_exit_b(site, bytes)
                    };
                    HookKind::InlineCopy(InlineCopy {
                        dst_offset: dst,
                        src_offset: src,
                        len_offset: len,
                        exit_pc: exit,
                        spill_back: *spill_back,
                    })
                }
                PatternHookKind::RegInlineCopy { count_offset } => {
                    let src = arm_register_from_index(
                        pm.src_reg
                            .ok_or_else(|| WieError::FatalError(format!("pattern match at {match_addr:#x} missing src register capture")))?,
                    );
                    let dst = arm_register_from_index(
                        pm.dst_reg
                            .ok_or_else(|| WieError::FatalError(format!("pattern match at {match_addr:#x} missing dst register capture")))?,
                    );
                    let count = arm_register_from_index(
                        pm.count_reg
                            .ok_or_else(|| WieError::FatalError(format!("pattern match at {match_addr:#x} missing count register capture")))?,
                    );
                    let exit_pc = match_addr.wrapping_add(pattern.tokens.len() as u32) | 1;
                    HookKind::RegInlineCopy(RegInlineCopy {
                        src,
                        dst,
                        count,
                        count_offset: *count_offset,
                        exit_pc,
                    })
                }
                PatternHookKind::Inotia1RevivalBaseRepair => HookKind::Inotia1RevivalBaseRepair,
            };
            let pc = match_addr | 1;
            if installed.iter().any(|h| h.pc == pc) {
                tracing::warn!("Hook at {pc:#x} already registered; skipping duplicate match");
                continue;
            }
            installed.push(Hook { pc, kind });
        }
    }

    Ok(installed)
}

/// Patch the SVC instruction at every hook PC and register the dispatcher.
/// `hooks` must already be the fully expanded list from `resolve_hooks`. The
/// dispatcher is registered even when `hooks` is empty so that any later SVC
/// #0x80 (e.g., from guest code that happens to encode the same bytes) routes
/// to a single, named diagnostic instead of falling out of the engine.
pub fn apply_hooks(core: &mut ArmCore, entry_name: &str, hooks: &[Hook]) -> Result<()> {
    let mut registry = BTreeMap::new();
    for hook in hooks {
        debug_assert!(hook.pc & 1 == 1, "resolve_hooks must reject ARM-mode PCs");
        registry.insert(hook.pc, hook.kind);
        let patch_addr = hook.pc & !1;
        let instruction: u16 = 0xdf00 | (BINARY_PATCH_SVC as u16 & 0xff);
        core.write_bytes(patch_addr, &instruction.to_le_bytes())?;
        tracing::info!("Hook installed at {:#x}: {:?}", hook.pc, hook.kind);
    }
    if !hooks.is_empty() {
        tracing::info!("Installed {} hooks for {}", hooks.len(), entry_name);
    }
    core.register_svc_handler(BINARY_PATCH_SVC, handle_binary_patch_svc, &Arc::new(registry))?;
    Ok(())
}

/// Negate the unsigned `SUBS Rn, #imm8` immediate: the captured byte is the
/// distance below R7, so the resulting offset is `-imm8`.
fn capture_to_offset(byte: u8) -> i32 {
    -(byte as i32)
}

fn arm_register_from_index(index: u8) -> ArmRegister {
    match index {
        0 => ArmRegister::R0,
        1 => ArmRegister::R1,
        2 => ArmRegister::R2,
        3 => ArmRegister::R3,
        4 => ArmRegister::R4,
        5 => ArmRegister::R5,
        6 => ArmRegister::R6,
        7 => ArmRegister::R7,
        _ => unreachable!("BitMatch only captures 3 bits, value must be 0..=7"),
    }
}

/// Decode a Thumb `B imm11` (`11100 iiiiiiiiiii`) at `b_site`. Returns the
/// target PC with the Thumb bit set.
fn decode_exit_b(b_site: u32, bytes: [u8; 2]) -> u32 {
    let raw = u16::from_le_bytes(bytes);
    let imm11 = (raw & 0x07ff) as i32;
    let offset = if imm11 & 0x400 != 0 { imm11 - 0x800 } else { imm11 };
    let target = (b_site.wrapping_add(4) as i64).wrapping_add((offset * 2) as i64) as u32;
    target | 1
}

type Registry = Arc<BTreeMap<u32, HookKind>>;

// Phase 8.32 — Inotia 1 persisted character-name recovery.
//
// Static analysis of the exact PD005362 native image resolves the command-30
// catalog parser's name-pointer table through GOT slot r10+0x4b8.  The table
// has twelve valid 4-byte entries.  Phase 8.28's former 18-record response
// wrote record 13 and record 14 into the two immediately-adjacent pointer
// slots at table+0x30 and table+0x34.  Field testing then confirmed those two
// pointers became the persisted scenario character names:
//
//   slot 0: 자원 교환권  -> 이노티아
//   slot 1: 초보용 용사의 인장 -> 기사
//
// Do not touch the opaque save database and do not scan arbitrary guest
// memory.  At ordinary compiler-library hooks, resolve the exact two adjacent
// slots using the title's live static-base register and repair *in place* only
// when the complete NUL-terminated byte sequence exactly matches one of the
// two known corrupt EUC-KR names.  The replacement strings are shorter, so
// zero-filling the old allocation is capacity-safe.  Once changed, the game's
// original save serializer can persist the corrected strings normally.
const INOTIA1_CASH_NAME_TABLE_GOT_OFFSET: u32 = 0x4b8;
const INOTIA1_CHARACTER_NAME0_POINTER_OFFSET: u32 = 12 * 4;
const INOTIA1_CHARACTER_NAME1_POINTER_OFFSET: u32 = 13 * 4;

const INOTIA1_CORRUPT_NAME0: [u8; 12] = [
    0xc0, 0xda, 0xbf, 0xf8, 0x20, 0xb1, 0xb3, 0xc8, 0xaf, 0xb1, 0xc7, 0x00,
];
const INOTIA1_CORRECT_NAME0_PADDED: [u8; 12] = [
    0xc0, 0xcc, 0xb3, 0xeb, 0xc6, 0xbc, 0xbe, 0xc6, 0x00, 0x00, 0x00, 0x00,
];
const INOTIA1_CORRUPT_NAME1: [u8; 19] = [
    0xc3, 0xca, 0xba, 0xb8, 0xbf, 0xeb, 0x20, 0xbf, 0xeb, 0xbb, 0xe7, 0xc0, 0xc7, 0x20, 0xc0, 0xce, 0xc0, 0xe5, 0x00,
];
const INOTIA1_CORRECT_NAME1_PADDED: [u8; 19] = [
    0xb1, 0xe2, 0xbb, 0xe7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

fn phase8_32_try_repair_inotia1_character_names(core: &mut ArmCore) {
    let r10 = core.inner.lock().engine.reg_read(ArmRegister::SL);

    // A failed read simply means this is another binary/app or the Inotia 1
    // globals are not initialized yet.  Generic compiler hooks are shared by
    // multiple titles, so this helper must always fail closed.
    let name_table: u32 = match read_generic(core, r10.wrapping_add(INOTIA1_CASH_NAME_TABLE_GOT_OFFSET)) {
        Ok(value) => value,
        Err(_) => return,
    };
    if name_table < 0x1000 {
        return;
    }

    let slots = [
        (
            0u8,
            name_table.wrapping_add(INOTIA1_CHARACTER_NAME0_POINTER_OFFSET),
            &INOTIA1_CORRUPT_NAME0[..],
            &INOTIA1_CORRECT_NAME0_PADDED[..],
            "자원 교환권",
            "이노티아",
        ),
        (
            1u8,
            name_table.wrapping_add(INOTIA1_CHARACTER_NAME1_POINTER_OFFSET),
            &INOTIA1_CORRUPT_NAME1[..],
            &INOTIA1_CORRECT_NAME1_PADDED[..],
            "초보용 용사의 인장",
            "기사",
        ),
    ];

    for (slot, pointer_slot, corrupt, replacement, old_text, new_text) in slots {
        let string_ptr: u32 = match read_generic(core, pointer_slot) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if string_ptr < 0x1000 {
            continue;
        }

        let mut current = [0u8; 19];
        let current = &mut current[..corrupt.len()];
        if core.read_bytes(string_ptr, current).is_err() || &*current != corrupt {
            continue;
        }
        if core.write_bytes(string_ptr, replacement).is_err() {
            continue;
        }

        tracing::info!(
            "[PHASE8_32_INOTIA1_CHARACTER_NAME_REPAIR] slot={slot} pointer_slot={pointer_slot:#010x} string_ptr={string_ptr:#010x} {old_text:?} -> {new_text:?}; exact-match in-place repair applied"
        );
    }
}

// Phase 8.34 — runtime name-string diagnostic at the exact Inotia 1 RVCT
// strcpy/strlen hooks. Phase 8.33 found no persistent small-heap copies at
// cash-shop open, so the corrupt scenario labels may exist only transiently
// while the character screen formats them. Log exact base/display matches with
// LR. A complete "name(class)" match is unambiguous and safe to repair in
// place; base-name matches are diagnostic only because cash items can legally
// use those same labels.
const INOTIA1_CORRUPT_NAME0_DISPLAY_P834: [u8; 18] = [
    0xc0, 0xda, 0xbf, 0xf8, 0x20, 0xb1, 0xb3, 0xc8, 0xaf, 0xb1, 0xc7, 0x28,
    0xb5, 0xb5, 0xc0, 0xfb, 0x29, 0x00,
];
const INOTIA1_CORRECT_NAME0_DISPLAY_P834: [u8; 18] = [
    0xc0, 0xcc, 0xb3, 0xeb, 0xc6, 0xbc, 0xbe, 0xc6, 0x28, 0xb5, 0xb5, 0xc0,
    0xfb, 0x29, 0x00, 0x00, 0x00, 0x00,
];
const INOTIA1_CORRUPT_NAME1_DISPLAY_P834: [u8; 25] = [
    0xc3, 0xca, 0xba, 0xb8, 0xbf, 0xeb, 0x20, 0xbf, 0xeb, 0xbb, 0xe7, 0xc0,
    0xc7, 0x20, 0xc0, 0xce, 0xc0, 0xe5, 0x28, 0xb1, 0xe2, 0xbb, 0xe7, 0x29,
    0x00,
];
const INOTIA1_CORRECT_NAME1_DISPLAY_P834: [u8; 25] = [
    0xb1, 0xe2, 0xbb, 0xe7, 0x28, 0xb1, 0xe2, 0xbb, 0xe7, 0x29, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00,
];


// Phase 8.36 — low-overhead Inotia 1 main-character name recovery.
//
// Phase 8.35 confirmed the visible repair works, but the broad Phase 8.34
// tracing ran on generic strcpy/strlen hot paths and made normal gameplay
// noticeably slower.  The field log also gave us the exact Inotia 1 callers:
//   0x0010cf29: main base-name strlen while scenario/character text is built
//   0x0010cf5b: full "name(class)" strlen before display
// Check only these two callers and only the exact corrupt main-character bytes.
// No secondary-hero repair, heap scan, GOT walk, or per-call diagnostic logging.
fn phase8_36_try_repair_inotia1_main_name_at_strlen(core: &mut ArmCore, ptr: u32, lr: u32) {
    const MAIN_BASE_CALLER: u32 = 0x0010_cf29;
    const MAIN_DISPLAY_CALLER: u32 = 0x0010_cf5b;

    if lr == MAIN_BASE_CALLER {
        let mut current = [0u8; 12];
        if core.read_bytes(ptr, &mut current).is_ok() && current == INOTIA1_CORRUPT_NAME0 {
            if core.write_bytes(ptr, &INOTIA1_CORRECT_NAME0_PADDED).is_ok() {
                tracing::info!(
                    "[PHASE8_36_INOTIA1_MAIN_NAME_CALLSITE_REPAIR] form=base ptr={ptr:#010x} lr={lr:#010x} 자원 교환권 -> 이노티아"
                );
            }
        }
    } else if lr == MAIN_DISPLAY_CALLER {
        let mut current = [0u8; 18];
        if core.read_bytes(ptr, &mut current).is_ok()
            && current == INOTIA1_CORRUPT_NAME0_DISPLAY_P834
        {
            if core
                .write_bytes(ptr, &INOTIA1_CORRECT_NAME0_DISPLAY_P834)
                .is_ok()
            {
                tracing::info!(
                    "[PHASE8_36_INOTIA1_MAIN_NAME_CALLSITE_REPAIR] form=display ptr={ptr:#010x} lr={lr:#010x} 자원 교환권(도적) -> 이노티아(도적)"
                );
            }
        }
    }
}

fn phase8_34_trace_inotia1_name_string(core: &mut ArmCore, kind: &str, ptr: u32, lr: u32) {
    let mut bytes = [0u8; 25];
    if core.read_bytes(ptr, &mut bytes).is_err() {
        return;
    }

    if &bytes[..INOTIA1_CORRUPT_NAME0_DISPLAY_P834.len()] == &INOTIA1_CORRUPT_NAME0_DISPLAY_P834 {
        tracing::info!(
            "[PHASE8_34_INOTIA1_NAME_STRING_CALL] kind={kind} form=display0 ptr={ptr:#010x} lr={lr:#010x}"
        );
        if core.write_bytes(ptr, &INOTIA1_CORRECT_NAME0_DISPLAY_P834).is_ok() {
            tracing::info!(
                "[PHASE8_34_INOTIA1_DISPLAY_NAME_REPAIR] slot=0 ptr={ptr:#010x} lr={lr:#010x} 자원 교환권(도적) -> 이노티아(도적)"
            );
        }
        return;
    }
    if &bytes[..INOTIA1_CORRUPT_NAME1_DISPLAY_P834.len()] == &INOTIA1_CORRUPT_NAME1_DISPLAY_P834 {
        tracing::info!(
            "[PHASE8_34_INOTIA1_NAME_STRING_CALL] kind={kind} form=display1 ptr={ptr:#010x} lr={lr:#010x}"
        );
        if core.write_bytes(ptr, &INOTIA1_CORRECT_NAME1_DISPLAY_P834).is_ok() {
            tracing::info!(
                "[PHASE8_34_INOTIA1_DISPLAY_NAME_REPAIR] slot=1 ptr={ptr:#010x} lr={lr:#010x} 초보용 용사의 인장(기사) -> 기사(기사)"
            );
        }
        return;
    }

    if &bytes[..INOTIA1_CORRUPT_NAME0.len()] == &INOTIA1_CORRUPT_NAME0 {
        tracing::info!(
            "[PHASE8_34_INOTIA1_NAME_STRING_CALL] kind={kind} form=base0 ptr={ptr:#010x} lr={lr:#010x}; diagnostic-only"
        );
    } else if &bytes[..INOTIA1_CORRUPT_NAME1.len()] == &INOTIA1_CORRUPT_NAME1 {
        tracing::info!(
            "[PHASE8_34_INOTIA1_NAME_STRING_CALL] kind={kind} form=base1 ptr={ptr:#010x} lr={lr:#010x}; diagnostic-only"
        );
    }
}


/// Exact reconstruction of the original Inotia1 helper at guest 0x001281ec.
/// All multiplies/adds wrap to 32 bits and the final division is signed,
/// matching the Thumb code and its signed division helper.
fn inotia1_reward_original_wrapped(a: u32, _b: u32, c: u32, d: u32, e: u32, f: u32) -> Option<i32> {
    let hundred_minus_f = 100u32.wrapping_sub(f);

    let x = d
        .wrapping_add(a.wrapping_mul(100))
        .wrapping_add(1000);
    let mut numerator = c.wrapping_mul(hundred_minus_f).wrapping_mul(x);

    let y = a
        .wrapping_mul(50)
        .wrapping_add(d)
        .wrapping_mul(2)
        .wrapping_add(1000);
    numerator = numerator.wrapping_add(y.wrapping_mul(f));
    numerator = numerator.wrapping_add(e.wrapping_mul(hundred_minus_f).wrapping_mul(y));

    let denominator = hundred_minus_f.wrapping_mul(y);
    let signed_denominator = denominator as i32 as i64;
    if signed_denominator == 0 {
        return None;
    }
    let signed_numerator = numerator as i32 as i64;
    Some((signed_numerator / signed_denominator) as i32)
}

/// Mathematically equivalent reward formula using wide, non-wrapping
/// intermediates. Returns None for inputs outside the constructor's sensible
/// domain or if the final reward cannot be represented as a positive i32.
fn inotia1_reward_wide(a: u32, _b: u32, c: u32, d: u32, e: u32, f: u32) -> Option<(i32, u128, u128)> {
    if f >= 100 {
        return None;
    }
    let a = a as u128;
    let c = c as u128;
    let d = d as u128;
    let e = e as u128;
    let f = f as u128;
    let hundred_minus_f = 100u128 - f;

    let x = d + a * 100 + 1000;
    let y = (a * 50 + d) * 2 + 1000;
    let numerator = c * hundred_minus_f * x + y * f + e * hundred_minus_f * y;
    let denominator = hundred_minus_f * y;
    if denominator == 0 {
        return None;
    }
    let reward = numerator / denominator;
    if reward > i32::MAX as u128 {
        return None;
    }
    Some((reward as i32, numerator, denominator))
}

async fn handle_binary_patch_svc(core: &mut ArmCore, registry: &mut Registry) -> Result<JumpTo> {
    let (pc, lr) = core.read_pc_lr()?;
    // PC on entry is the address right after the patched 2-byte SVC. Drop any
    // Thumb bit, step back over the SVC, then re-set the Thumb bit because
    // hook PCs are stored that way.
    let hook_pc = (pc.wrapping_sub(2)) | 1;
    let kind = registry
        .get(&hook_pc)
        .copied()
        .ok_or_else(|| WieError::FatalError(format!("binary-patch hook fired at unregistered PC {hook_pc:#x}")))?;

    match kind {
        HookKind::Memcpy => {
            let (dst, src, len) = {
                let inner = core.inner.lock();
                (
                    inner.engine.reg_read(ArmRegister::R0),
                    inner.engine.reg_read(ArmRegister::R1),
                    inner.engine.reg_read(ArmRegister::R2),
                )
            };
            tracing::trace!("hook memcpy(ptr_dst={dst:#x}, ptr_src={src:#x}, len={len:#x})");
            stdlib::memcpy(core, &mut (), dst, src, len).await?;
            Ok(JumpTo(lr))
        }
        HookKind::Memset => {
            let (dst, val, len) = {
                let inner = core.inner.lock();
                (
                    inner.engine.reg_read(ArmRegister::R0),
                    inner.engine.reg_read(ArmRegister::R1),
                    inner.engine.reg_read(ArmRegister::R2),
                )
            };
            tracing::trace!("hook memset(ptr_dst={dst:#x}, val={:#x}, len={len:#x})", val as u8);
            stdlib::memset(core, &mut (), dst, val, len).await?;
            Ok(JumpTo(lr))
        }
        HookKind::Strcpy => {
            let (dst, src) = {
                let inner = core.inner.lock();
                (inner.engine.reg_read(ArmRegister::R0), inner.engine.reg_read(ArmRegister::R1))
            };
            tracing::trace!("hook strcpy(ptr_dst={dst:#x}, ptr_src={src:#x})");
            stdlib::strcpy(core, &mut (), dst, src).await?;
            Ok(JumpTo(lr))
        }
        HookKind::Strlen => {
            let s = core.inner.lock().engine.reg_read(ArmRegister::R0);
            if hook_pc == 0x0015_433d {
                phase8_36_try_repair_inotia1_main_name_at_strlen(core, s, lr);
            }
            let len = stdlib::strlen(core, &mut (), s).await?;
            tracing::trace!("hook strlen(ptr_str={s:#x}) -> {len:#x}");
            core.inner.lock().engine.reg_write(ArmRegister::R0, len);
            Ok(JumpTo(lr))
        }
        HookKind::InlineCopy(spec) => {
            let r7 = core.inner.lock().engine.reg_read(ArmRegister::R7);
            let dst_slot = r7.wrapping_add(spec.dst_offset as u32);
            let src_slot = r7.wrapping_add(spec.src_offset as u32);
            let len_slot = r7.wrapping_add(spec.len_offset as u32);
            let dst: u32 = read_generic(core, dst_slot)?;
            let src: u32 = read_generic(core, src_slot)?;
            let len: u32 = read_generic(core, len_slot)?;
            tracing::trace!(
                "hook inline_copy(ptr_dst={dst:#x}, ptr_src={src:#x}, len={len:#x}, exit={:#x})",
                spec.exit_pc
            );
            stdlib::memcpy(core, &mut (), dst, src, len).await?;
            if spec.spill_back {
                core.write_bytes(dst_slot, &dst.wrapping_add(len).to_le_bytes())?;
                core.write_bytes(src_slot, &src.wrapping_add(len).to_le_bytes())?;
                core.write_bytes(len_slot, &0u32.to_le_bytes())?;
            }
            Ok(JumpTo(spec.exit_pc))
        }
        HookKind::RegInlineCopy(spec) => {
            let (src, dst, count_initial) = {
                let inner = core.inner.lock();
                (
                    inner.engine.reg_read(spec.src),
                    inner.engine.reg_read(spec.dst),
                    inner.engine.reg_read(spec.count),
                )
            };
            let count = count_initial.wrapping_add(spec.count_offset as u32);
            tracing::trace!(
                "hook reg_inline_copy(src={src:#x}, dst={dst:#x}, count={count:#x}, exit={:#x})",
                spec.exit_pc
            );
            stdlib::memcpy(core, &mut (), dst, src, count).await?;
            let mut inner = core.inner.lock();
            inner.engine.reg_write(spec.src, src.wrapping_add(count));
            inner.engine.reg_write(spec.dst, dst.wrapping_add(count));
            inner.engine.reg_write(spec.count, count_initial.wrapping_sub(count));
            Ok(JumpTo(spec.exit_pc))
        }
        HookKind::Inotia1RewardWideMathRepair => {
            // ABI at entry to 0x001281ec:
            //   r0=a, r1=b (unused by the original helper), r2=c, r3=d,
            //   [sp]=e, [sp+4]=f.  The monster constructor calls from
            //   LR=0x00126245 and immediately stores R0 to entity+0x00.
            let (a, b, c, d, sp) = {
                let inner = core.inner.lock();
                (
                    inner.engine.reg_read(ArmRegister::R0),
                    inner.engine.reg_read(ArmRegister::R1),
                    inner.engine.reg_read(ArmRegister::R2),
                    inner.engine.reg_read(ArmRegister::R3),
                    inner.engine.reg_read(ArmRegister::SP),
                )
            };
            let e: u32 = read_generic(core, sp)?;
            let f: u32 = read_generic(core, sp.wrapping_add(4))?;
            let original = inotia1_reward_original_wrapped(a, b, c, d, e, f);
            let wide = inotia1_reward_wide(a, b, c, d, e, f);

            // This helper may theoretically be reused elsewhere. Repair only
            // the exact monster-constructor caller discovered in Phase 8.48;
            // for any other caller, faithfully return the original wrapped
            // result reconstructed above.
            const INOTIA1_MONSTER_REWARD_CALLER_LR: u32 = 0x0012_6245;
            let constructor_call = lr == INOTIA1_MONSTER_REWARD_CALLER_LR;
            let original_value = original.unwrap_or(0);
            let mut applied_value = original_value;
            let mut repaired = false;
            let mut wide_numerator = 0u128;
            let mut wide_denominator = 0u128;
            let mut corrected_value = original_value;

            if let Some((corrected, numerator, denominator)) = wide {
                corrected_value = corrected;
                wide_numerator = numerator;
                wide_denominator = denominator;
                if constructor_call && original == Some(original_value) && corrected != original_value {
                    applied_value = corrected;
                    repaired = true;
                }
            }

            core.inner
                .lock()
                .engine
                .reg_write(ArmRegister::R0, applied_value as u32);

            // Phase 8.53 cleanup: normal monster spawns are intentionally silent.
            // Emit one concise marker only when the wide result actually repairs
            // a wrapped 32-bit reward. This preserves field visibility without
            // flooding the session log for every unaffected monster.
            if repaired {
                tracing::info!(
                    "[PHASE8_53_INOTIA1_REWARD_OVERFLOW_REPAIR] lr={lr:#010x} original={} corrected={} original_hex={:#010x} corrected_hex={:#010x} wide_numerator={} wide_denominator={} a={a} b={b} c={c} d={d} e={e} f={f}",
                    original_value,
                    corrected_value,
                    original_value as u32,
                    corrected_value as u32,
                    wide_numerator,
                    wide_denominator,
                );
            }
            Ok(JumpTo(lr))
        }
        HookKind::Inotia1RevivalBaseRepair => {
            // This hook is selected by an exact-title byte pattern beginning at
            // guest 0x00131cb2. Preserve the original `LDR R0,[SP,#0x4c]`,
            // then repair only the context register consumed by the immediately
            // following 0x0011dfb8 call.
            let (sp, r8, old_r4) = {
                let inner = core.inner.lock();
                (
                    inner.engine.reg_read(ArmRegister::SP),
                    inner.engine.reg_read(ArmRegister::R8),
                    inner.engine.reg_read(ArmRegister::R4),
                )
            };
            let arg0: u32 = read_generic(core, sp.wrapping_add(0x4c))?;

            // Fail closed if R8 does not actually look like the structure
            // observed by static analysis. The legacy behavior then continues
            // unchanged and the exception-only fault trace remains available.
            let field_248: Result<u32> = read_generic(core, r8.wrapping_add(0x248));
            let field_24c: Result<u32> = read_generic(core, r8.wrapping_add(0x24c));
            let repaired = field_248.is_ok() && field_24c.is_ok();
            {
                let mut inner = core.inner.lock();
                inner.engine.reg_write(ArmRegister::R0, arg0);
                if repaired {
                    inner.engine.reg_write(ArmRegister::R4, r8);
                }
            }
            tracing::info!(
                "[PHASE8_40_INOTIA1_REVIVAL_CONTEXT_REPAIR] hook={hook_pc:#010x} lr={lr:#010x} sp={sp:#010x} arg0={arg0:#010x} old_r4={old_r4:#010x} r8={r8:#010x} field248_ok={} field24c_ok={} repaired={repaired}",
                field_248.is_ok(),
                field_24c.is_ok()
            );

            // The replaced Thumb instruction is exactly two bytes long.
            Ok(JumpTo(((hook_pc & !1).wrapping_add(2)) | 1))
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::{vec, vec::Vec};

    use wie_util::ByteRead;

    use super::*;
    use crate::function::{RegisteredFunction, RegisteredFunctionHolder};

    fn registry_with(pc: u32, kind: HookKind) -> Registry {
        let mut map = BTreeMap::new();
        map.insert(pc, kind);
        Arc::new(map)
    }

    /// Set PC to where it would be on entry to the SVC handler for a hook at
    /// `hook_pc` (svc_addr + 2, i.e. just past the patched 2-byte SVC).
    fn set_post_svc_pc(core: &mut ArmCore, hook_pc: u32) {
        let mut inner = core.inner.lock();
        inner.engine.reg_write(ArmRegister::PC, (hook_pc & !1).wrapping_add(2));
    }

    fn entry_with_static(name: &str, hooks: Vec<Hook>) -> Entry {
        Entry {
            hash: Some([0u8; 16]),
            name: name.into(),
            hooks,
            hook_patterns: vec![],
            patches: vec![],
            patch_patterns: vec![],
        }
    }

    #[test]
    fn phase8_32_inotia1_character_name_repair_is_exact_and_in_place() -> Result<()> {
        let mut core = ArmCore::new(false, None)?;
        core.map(0x10000, 0x3000)?;

        let r10 = 0x10000u32;
        let name_table = 0x10800u32;
        let name0 = 0x10900u32;
        let name1 = 0x10940u32;
        {
            let mut inner = core.inner.lock();
            inner.engine.reg_write(ArmRegister::SL, r10);
        }
        core.write_bytes(r10 + INOTIA1_CASH_NAME_TABLE_GOT_OFFSET, &name_table.to_le_bytes())?;
        core.write_bytes(name_table + INOTIA1_CHARACTER_NAME0_POINTER_OFFSET, &name0.to_le_bytes())?;
        core.write_bytes(name_table + INOTIA1_CHARACTER_NAME1_POINTER_OFFSET, &name1.to_le_bytes())?;
        core.write_bytes(name0, &INOTIA1_CORRUPT_NAME0)?;
        core.write_bytes(name1, &INOTIA1_CORRUPT_NAME1)?;

        phase8_32_try_repair_inotia1_character_names(&mut core);

        let mut out0 = [0xffu8; 12];
        let mut out1 = [0xffu8; 19];
        core.read_bytes(name0, &mut out0)?;
        core.read_bytes(name1, &mut out1)?;
        assert_eq!(out0, INOTIA1_CORRECT_NAME0_PADDED);
        assert_eq!(out1, INOTIA1_CORRECT_NAME1_PADDED);
        Ok(())
    }

    #[test]
    fn phase8_36_main_name_callsite_repair_is_exact_and_caller_bounded() -> Result<()> {
        let mut core = ArmCore::new(false, None)?;
        core.map(0x20000, 0x2000)?;

        let base_ptr = 0x20400u32;
        core.write_bytes(base_ptr, &INOTIA1_CORRUPT_NAME0)?;
        phase8_36_try_repair_inotia1_main_name_at_strlen(&mut core, base_ptr, 0x0010_cf29);
        let mut base_out = [0xffu8; 12];
        core.read_bytes(base_ptr, &mut base_out)?;
        assert_eq!(base_out, INOTIA1_CORRECT_NAME0_PADDED);

        let display_ptr = 0x20500u32;
        core.write_bytes(display_ptr, &INOTIA1_CORRUPT_NAME0_DISPLAY_P834)?;
        phase8_36_try_repair_inotia1_main_name_at_strlen(&mut core, display_ptr, 0x0010_cf5b);
        let mut display_out = [0xffu8; 18];
        core.read_bytes(display_ptr, &mut display_out)?;
        assert_eq!(display_out, INOTIA1_CORRECT_NAME0_DISPLAY_P834);

        let wrong_caller_ptr = 0x20600u32;
        core.write_bytes(wrong_caller_ptr, &INOTIA1_CORRUPT_NAME0)?;
        phase8_36_try_repair_inotia1_main_name_at_strlen(&mut core, wrong_caller_ptr, 0x0010_cf5b);
        let mut unchanged = [0u8; 12];
        core.read_bytes(wrong_caller_ptr, &mut unchanged)?;
        assert_eq!(unchanged, INOTIA1_CORRUPT_NAME0);
        Ok(())
    }

    #[test]
    fn phase8_49_inotia1_reward_wide_math_preserves_non_overflowing_monster() {
        // 수호자 C44, captured by Phase 8.48.
        let original = inotia1_reward_original_wrapped(38, 233, 3690, 960, 10, 3);
        let wide = inotia1_reward_wide(38, 233, 3690, 960, 10, 3);
        assert_eq!(original, Some(3172));
        assert_eq!(wide.map(|v| v.0), Some(3172));
    }

    #[test]
    fn phase8_49_inotia1_reward_wide_math_repairs_overflowing_monster() {
        // 수호물 K34, captured by Phase 8.48. Original signed 32-bit arithmetic
        // wraps the numerator to -1,978,494,016 and produces -3035. Wide math
        // preserves the intended positive numerator and produces 3553.
        let original = inotia1_reward_original_wrapped(38, 212, 4132, 960, 12, 3);
        let wide = inotia1_reward_wide(38, 212, 4132, 960, 12, 3);
        assert_eq!(original, Some(-3035));
        assert_eq!(wide, Some((3553, 2_316_473_280, 651_840)));
    }

    #[test]
    fn phase8_49_inotia1_reward_wide_math_repairs_second_overflow_family() {
        // Another overflow family observed in the same Phase 8.48 spawn log.
        let original = inotia1_reward_original_wrapped(41, 239, 4789, 1020, 12, 3);
        let wide = inotia1_reward_wide(41, 239, 4789, 1020, 12, 3);
        assert_eq!(original, Some(-2084));
        assert_eq!(wide.map(|v| v.0), Some(4116));
    }

    #[test]
    fn resolve_hooks_rejects_arm_mode_pc() -> Result<()> {
        let entry = entry_with_static(
            "arm-mode",
            vec![Hook {
                pc: 0x2000, // LSB=0 => ARM mode
                kind: HookKind::Memcpy,
            }],
        );
        let mut core = ArmCore::new(false, None)?;
        core.map(0x2000, 0x1000)?;

        let err = resolve_hooks(&mut core, &entry, &[]).unwrap_err();
        let msg = alloc::format!("{err}");
        assert!(msg.contains("ARM mode"), "unexpected error: {msg}");
        Ok(())
    }

    #[test]
    fn apply_hooks_writes_thumb_svc_instruction() -> Result<()> {
        let entry = entry_with_static(
            "patch",
            vec![Hook {
                pc: 0x2001, // Thumb
                kind: HookKind::Memcpy,
            }],
        );
        let mut core = ArmCore::new(false, None)?;
        core.map(0x2000, 0x1000)?;
        core.write_bytes(0x2000, &[0xaa, 0xbb])?;

        let hooks = resolve_hooks(&mut core, &entry, &[])?;
        apply_hooks(&mut core, &entry.name, &hooks)?;

        let mut buf = [0u8; 2];
        core.read_bytes(0x2000, &mut buf)?;
        assert_eq!(buf, [BINARY_PATCH_SVC as u8, 0xdf]);
        Ok(())
    }

    #[futures_test::test]
    async fn memcpy_dispatch_copies_bytes_and_returns_via_lr() -> Result<()> {
        let mut core = ArmCore::new(false, None)?;
        core.map(0x10000, 0x1000)?;

        let src = 0x10000u32;
        let dst = 0x10400u32;
        let data = [1u8, 2, 3, 4, 5, 6, 7, 8];
        core.write_bytes(src, &data)?;

        let hook_pc = 0x10001u32;
        {
            let mut inner = core.inner.lock();
            inner.engine.reg_write(ArmRegister::R0, dst);
            inner.engine.reg_write(ArmRegister::R1, src);
            inner.engine.reg_write(ArmRegister::R2, data.len() as u32);
            inner.engine.reg_write(ArmRegister::LR, 0xdead_beef);
        }
        set_post_svc_pc(&mut core, hook_pc);

        let registry = registry_with(hook_pc, HookKind::Memcpy);
        RegisteredFunctionHolder::new(handle_binary_patch_svc, &registry).call(&mut core).await?;

        let mut out = [0u8; 8];
        core.read_bytes(dst, &mut out)?;
        assert_eq!(out, data);
        assert_eq!(core.inner.lock().engine.reg_read(ArmRegister::PC), 0xdead_beef & !1);
        Ok(())
    }

    #[futures_test::test]
    async fn memset_dispatch_fills_bytes() -> Result<()> {
        let mut core = ArmCore::new(false, None)?;
        core.map(0x10000, 0x1000)?;

        let dst = 0x10000u32;
        let len = 16u32;
        let hook_pc = 0x10401u32;
        {
            let mut inner = core.inner.lock();
            inner.engine.reg_write(ArmRegister::R0, dst);
            inner.engine.reg_write(ArmRegister::R1, 0xab);
            inner.engine.reg_write(ArmRegister::R2, len);
            inner.engine.reg_write(ArmRegister::LR, 0x3000);
        }
        set_post_svc_pc(&mut core, hook_pc);

        let registry = registry_with(hook_pc, HookKind::Memset);
        RegisteredFunctionHolder::new(handle_binary_patch_svc, &registry).call(&mut core).await?;

        let mut out = [0u8; 16];
        core.read_bytes(dst, &mut out)?;
        assert_eq!(out, [0xab; 16]);
        Ok(())
    }

    #[futures_test::test]
    async fn strcpy_dispatch_copies_null_terminated_string_and_returns_via_lr() -> Result<()> {
        let mut core = ArmCore::new(false, None)?;
        core.map(0x10000, 0x1000)?;

        let src = 0x10000u32;
        let dst = 0x10400u32;
        let s = b"hello, world!\0";
        core.write_bytes(src, s)?;
        core.write_bytes(dst, &[0xff; 32])?;

        let hook_pc = 0x10801u32;
        {
            let mut inner = core.inner.lock();
            inner.engine.reg_write(ArmRegister::R0, dst);
            inner.engine.reg_write(ArmRegister::R1, src);
            inner.engine.reg_write(ArmRegister::LR, 0xcafe_babe);
        }
        set_post_svc_pc(&mut core, hook_pc);

        let registry = registry_with(hook_pc, HookKind::Strcpy);
        RegisteredFunctionHolder::new(handle_binary_patch_svc, &registry).call(&mut core).await?;

        let mut out = [0u8; 14];
        core.read_bytes(dst, &mut out)?;
        assert_eq!(&out, s);
        let mut sentinel = [0u8; 1];
        core.read_bytes(dst + s.len() as u32, &mut sentinel)?;
        assert_eq!(sentinel, [0xff]);

        let inner = core.inner.lock();
        assert_eq!(inner.engine.reg_read(ArmRegister::R0), dst);
        assert_eq!(inner.engine.reg_read(ArmRegister::PC), 0xcafe_babe & !1);
        Ok(())
    }

    #[futures_test::test]
    async fn strlen_dispatch_returns_length_in_r0() -> Result<()> {
        let mut core = ArmCore::new(false, None)?;
        core.map(0x10000, 0x1000)?;

        let str_ptr = 0x10100u32;
        let s = b"abcdef\0";
        core.write_bytes(str_ptr, s)?;

        let hook_pc = 0x10c01u32;
        {
            let mut inner = core.inner.lock();
            inner.engine.reg_write(ArmRegister::R0, str_ptr);
            inner.engine.reg_write(ArmRegister::LR, 0x1234_5678);
        }
        set_post_svc_pc(&mut core, hook_pc);

        let registry = registry_with(hook_pc, HookKind::Strlen);
        RegisteredFunctionHolder::new(handle_binary_patch_svc, &registry).call(&mut core).await?;

        let inner = core.inner.lock();
        assert_eq!(inner.engine.reg_read(ArmRegister::R0), 6);
        assert_eq!(inner.engine.reg_read(ArmRegister::PC), 0x1234_5678 & !1);
        Ok(())
    }

    #[futures_test::test]
    async fn inline_copy_dispatch_reads_frame_copies_and_jumps_to_exit() -> Result<()> {
        let mut core = ArmCore::new(false, None)?;
        core.map(0x10000, 0x2000)?;

        let src = 0x10000u32;
        let dst = 0x10800u32;
        let payload = [0xaa, 0xbb, 0xcc, 0xdd];
        core.write_bytes(src, &payload)?;

        let frame = 0x11000u32;
        core.write_bytes(frame, &dst.to_le_bytes())?;
        core.write_bytes(frame + 4, &src.to_le_bytes())?;
        core.write_bytes(frame + 8, &(payload.len() as u32).to_le_bytes())?;

        let hook_pc = 0x10201u32;
        {
            let mut inner = core.inner.lock();
            inner.engine.reg_write(ArmRegister::R7, frame);
        }
        set_post_svc_pc(&mut core, hook_pc);

        let spec = InlineCopy {
            dst_offset: 0,
            src_offset: 4,
            len_offset: 8,
            exit_pc: 0x10401,
            spill_back: true,
        };
        let registry = registry_with(hook_pc, HookKind::InlineCopy(spec));
        RegisteredFunctionHolder::new(handle_binary_patch_svc, &registry).call(&mut core).await?;

        let mut out = [0u8; 4];
        core.read_bytes(dst, &mut out)?;
        assert_eq!(out, payload);

        let mut slot = [0u8; 4];
        core.read_bytes(frame, &mut slot)?;
        assert_eq!(u32::from_le_bytes(slot), dst + payload.len() as u32);
        core.read_bytes(frame + 4, &mut slot)?;
        assert_eq!(u32::from_le_bytes(slot), src + payload.len() as u32);
        core.read_bytes(frame + 8, &mut slot)?;
        assert_eq!(u32::from_le_bytes(slot), 0);

        assert_eq!(core.inner.lock().engine.reg_read(ArmRegister::PC), 0x10400);
        Ok(())
    }

    #[futures_test::test]
    async fn install_then_execute_hits_dispatcher_end_to_end() -> Result<()> {
        let mut core = ArmCore::new(false, None)?;
        core.map(0x20000, 0x2000)?;
        core.map(0x30000, 0x1000)?;

        let src = 0x30000u32;
        let dst = 0x30200u32;
        let payload = [9u8, 8, 7, 6];
        core.write_bytes(src, &payload)?;

        let hook_pc = 0x20001u32;
        let entry = entry_with_static(
            "e2e",
            vec![Hook {
                pc: hook_pc,
                kind: HookKind::Memcpy,
            }],
        );
        let hooks = resolve_hooks(&mut core, &entry, &[])?;
        apply_hooks(&mut core, &entry.name, &hooks)?;

        let mut opcode = [0u8; 2];
        core.read_bytes(0x20000, &mut opcode)?;
        assert_eq!(opcode[1], 0xdf);
        assert_eq!(opcode[0] as u32, BINARY_PATCH_SVC);

        let return_addr = 0x40000u32 | 1;
        {
            let mut inner = core.inner.lock();
            inner.engine.reg_write(ArmRegister::R0, dst);
            inner.engine.reg_write(ArmRegister::R1, src);
            inner.engine.reg_write(ArmRegister::R2, payload.len() as u32);
            inner.engine.reg_write(ArmRegister::LR, return_addr);
            inner.engine.reg_write(ArmRegister::PC, hook_pc);

            let cpsr = inner.engine.reg_read(ArmRegister::Cpsr);
            inner.engine.reg_write(ArmRegister::Cpsr, (cpsr & !0x3f) | 0x1f | 0x20);
            inner.engine.reg_write(ArmRegister::SP, 0x20f00);
        }

        let result = {
            let mut inner = core.inner.lock();
            inner.engine.run(0, 10)?
        };
        let category = match result {
            crate::engine::EngineRunResult::Svc { category, lr, spsr } => {
                let mut inner = core.inner.lock();
                inner.engine.reg_write(ArmRegister::Cpsr, spsr);
                inner.engine.reg_write(ArmRegister::PC, lr);
                category
            }
            _ => panic!("expected Svc"),
        };
        assert_eq!(category, BINARY_PATCH_SVC);

        let registry = registry_with(hook_pc, HookKind::Memcpy);
        let mut core_clone = core.clone();
        RegisteredFunctionHolder::new(handle_binary_patch_svc, &registry)
            .call(&mut core_clone)
            .await?;

        let mut out = [0u8; 4];
        core.read_bytes(dst, &mut out)?;
        assert_eq!(out, payload);

        let inner = core.inner.lock();
        assert_eq!(inner.engine.reg_read(ArmRegister::PC), return_addr & !1);
        Ok(())
    }

    #[test]
    fn pattern_scan_matches_single_hit() -> Result<()> {
        let mut core = ArmCore::new(false, None)?;
        core.map(0x50000, 0x200)?;

        let pat_bytes = [0xaa, 0xbb, 0xcc, 0xdd];
        let match_addr = 0x50020u32;
        core.write_bytes(match_addr, &pat_bytes)?;

        let entry = Entry {
            hash: None,
            name: "scan-single".into(),
            hooks: vec![],
            hook_patterns: vec![PatternHook {
                tokens: vec![
                    PatternToken::Literal(0xaa),
                    PatternToken::Literal(0xbb),
                    PatternToken::Literal(0xcc),
                    PatternToken::Literal(0xdd),
                ],
                kind_template: PatternHookKind::Memcpy,
            }],
            patches: vec![],
            patch_patterns: vec![],
        };

        let hooks = resolve_hooks(&mut core, &entry, &[(0x50000, 0x200)])?;
        apply_hooks(&mut core, &entry.name, &hooks)?;

        let mut out = [0u8; 2];
        core.read_bytes(match_addr, &mut out)?;
        assert_eq!(out[1], 0xdf);
        assert_eq!(out[0] as u32, BINARY_PATCH_SVC);
        Ok(())
    }

    #[test]
    fn pattern_scan_capture_to_offset_negates_imm8() {
        assert_eq!(capture_to_offset(0x08), -8);
        assert_eq!(capture_to_offset(0x0c), -12);
        // imm8 in `SUBS Rn, #imm8` is unsigned 0..=255, so high values must
        // negate to large negative offsets, not wrap through `i8`.
        assert_eq!(capture_to_offset(0x80), -128);
        assert_eq!(capture_to_offset(0xfc), -252);
    }

    #[test]
    fn pattern_scan_exit_b_computes_forward_branch_target() {
        let bytes = [0x02, 0xe0];
        let site = 0x100u32;
        let exit = decode_exit_b(site, bytes);
        assert_eq!(exit, (site + 4 + 4) | 1);

        let neg = decode_exit_b(site, [0xfe, 0xe7]);
        assert_eq!(neg, site | 1);
    }

    #[test]
    fn pattern_duplicate_pc_warns_once_and_skips() -> Result<()> {
        let mut core = ArmCore::new(false, None)?;
        core.map(0x60000, 0x100)?;
        core.write_bytes(0x60010, &[0x11, 0x22, 0x33, 0x44])?;

        let entry = Entry {
            hash: None,
            name: "dup".into(),
            hooks: vec![],
            hook_patterns: vec![
                PatternHook {
                    tokens: vec![PatternToken::Literal(0x11), PatternToken::Literal(0x22)],
                    kind_template: PatternHookKind::Memcpy,
                },
                PatternHook {
                    tokens: vec![PatternToken::Literal(0x11), PatternToken::Literal(0x22)],
                    kind_template: PatternHookKind::Memcpy,
                },
            ],
            patches: vec![],
            patch_patterns: vec![],
        };
        let hooks = resolve_hooks(&mut core, &entry, &[(0x60000, 0x100)])?;
        assert_eq!(hooks.len(), 1, "duplicate PC should be skipped");
        Ok(())
    }
}
