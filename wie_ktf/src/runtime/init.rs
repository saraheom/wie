use alloc::{format, string::String, sync::Arc, vec, vec::Vec};
use core::mem::size_of;
use jvm::Jvm;

use wie_backend::System;
use wie_core_arm::{Allocator, ArmCore, EmulatedFunction, JumpTo, ResultWriter, SvcId};
use wie_util::{ByteRead, ByteWrite, Result, WieError, read_generic, read_null_terminated_string_bytes, write_generic};

use wipi_types::ktf::{ExeInterface, ExeInterfaceFunctions, InitParam0, InitParam1, InitParam3, InitParam4, WipiExe};

use crate::{
    adf::parse_bss_size,
    emulator::IMAGE_BASE,
    runtime::{
        SVC_CATEGORY_INIT,
        java::interface::{get_wipi_jb_interface, java_array_new, java_check_type, java_class_load, java_new, java_throw},
        svc_ids::InitSvcId,
        wipi_c::{interface::get_wipic_knl_interface, register_wipic_svc_handler},
    },
};

pub fn register_init_svc_handler(core: &mut ArmCore, jvm: &Jvm) -> Result<()> {
    core.register_svc_handler(SVC_CATEGORY_INIT, handle_init_svc, jvm)
}

async fn handle_init_svc(core: &mut ArmCore, jvm: &mut Jvm, id: SvcId) -> Result<()> {
    let (_, lr) = core.read_pc_lr()?;

    match InitSvcId::try_from(id)? {
        InitSvcId::GetInterface => get_interface(core, core.read_param(0)?).await?.write(core, lr),
        InitSvcId::JavaThrow => EmulatedFunction::call(&java_throw, core, jvm).await?.write(core, lr),
        InitSvcId::JavaCheckType => EmulatedFunction::call(&java_check_type, core, jvm).await?.write(core, lr),
        InitSvcId::JavaNew => EmulatedFunction::call(&java_new, core, jvm).await?.write(core, lr),
        InitSvcId::JavaArrayNew => EmulatedFunction::call(&java_array_new, core, jvm).await?.write(core, lr),
        InitSvcId::JavaClassLoad => EmulatedFunction::call(&java_class_load, core, jvm).await?.write(core, lr),
        InitSvcId::Alloc => EmulatedFunction::call(&alloc, core, &mut ()).await?.write(core, lr),
        InitSvcId::IncMem0 => inc_mem_slot(core, 0).await?.write(core, lr),
        InitSvcId::IncMem1 => inc_mem_slot(core, 1).await?.write(core, lr),
        InitSvcId::IncMem2 => inc_mem_slot(core, 2).await?.write(core, lr),
        InitSvcId::IncMem3 => inc_mem_slot(core, 3).await?.write(core, lr),
        InitSvcId::IncMem4 => inc_mem_slot(core, 4).await?.write(core, lr),
        InitSvcId::IncMem5 => inc_mem_slot(core, 5).await?.write(core, lr),
        InitSvcId::IncMem6 => inc_mem_slot(core, 6).await?.write(core, lr),
        InitSvcId::IncMem7 => inc_mem_slot(core, 7).await?.write(core, lr),
    }
}

// Phase 8.28 — batched exact-title host acceleration for Inotia 2's RGB565
// software effects.
//
// Phase 8.25 proved that replacing the inner RGB565 lookup/blend loop makes
// normal gameplay substantially smoother. The Phase 8.27 field test also
// showed why enabling all three graphics options can still regress: the old
// hook accelerated only *one row per SVC*. With weather/shadow/critical effects
// enabled, the title invokes this path for many more rows and therefore pays
// repeated SVC dispatch, mask lookup, 1 KiB LUT copy, and Vec allocation costs.
//
// The original guest outer loop is fully known at 0x00123f84..0x00123f98:
// sp+0x30 is the current row counter, sp+0x38 is total rows, and sp+0x10 is the
// byte stride. Process every remaining clipped row in one host call, cache the
// immutable 32x32 transform LUT for the launch, reuse one host pixel buffer,
// update the exact guest loop state, and resume at the original function
// epilogue. This preserves the game's effects while removing almost all
// per-row interpreter/SVC overhead.
//
// Exact AID/PID/native-size + original-byte install guards remain. Runtime
// RGB565 mask, dimensions, stride, and memory guards fall back to the original
// guest loop before modifying pixels if any invariant is unexpected.
const SVC_CATEGORY_INOTIA2_RGB565_EFFECT: u32 = 0x81;
const INOTIA2_RGB565_EFFECT_HOOK_ADDR: u32 = 0x0012_3f2a;
const INOTIA2_RGB565_EFFECT_HOOK_PC: u32 = 0x0012_3f2b;
const INOTIA2_RGB565_EFFECT_EPILOGUE_PC: u32 = 0x0012_3d4b;
const INOTIA2_RGB565_EFFECT_ORIGINAL: [u8; 2] = [0x02, 0x99]; // ldr r1,[sp,#8]
const INOTIA2_RGB565_EFFECT_SVC: [u8; 2] = [0x81, 0xdf];
const INOTIA2_RGB565_MAX_DIMENSION: u32 = 4096;
const INOTIA2_RGB565_MAX_BATCH_BYTES: usize = 32 * 1024 * 1024;

struct Inotia2Rgb565EffectState {
    lut_ptr: u32,
    lut_valid: bool,
    lut: [u8; 32 * 32],
    pixels: Vec<u8>,
    logged_first_batch: bool,
}

type Inotia2Rgb565EffectSharedState = Arc<spin::Mutex<Inotia2Rgb565EffectState>>;

fn disable_inotia2_rgb565_effect_fastpath(core: &mut ArmCore, reason: &str) -> Result<JumpTo> {
    core.write_bytes(
        INOTIA2_RGB565_EFFECT_HOOK_ADDR,
        &INOTIA2_RGB565_EFFECT_ORIGINAL,
    )?;
    tracing::warn!(
        "[PHASE8_28_INOTIA2_RGB565_BATCH] runtime gate failed ({reason}); original guest loop restored"
    );
    Ok(JumpTo(INOTIA2_RGB565_EFFECT_HOOK_PC))
}

fn handle_inotia2_rgb565_effect_batch(
    core: &mut ArmCore,
    shared: &Inotia2Rgb565EffectSharedState,
) -> Result<JumpTo> {
    let mut regs = core.save_context();
    let got = regs.r7;
    let sp = regs.sp;
    let row_ptr = regs.r6;
    let width = regs.r8;

    if width == 0 || width > INOTIA2_RGB565_MAX_DIMENSION {
        return disable_inotia2_rgb565_effect_fastpath(core, "invalid clipped width");
    }

    let current_row: u32 = match read_generic(core, sp.wrapping_add(0x30)) {
        Ok(value) => value,
        Err(_) => return disable_inotia2_rgb565_effect_fastpath(core, "row counter unavailable"),
    };
    let total_rows: u32 = match read_generic(core, sp.wrapping_add(0x38)) {
        Ok(value) => value,
        Err(_) => return disable_inotia2_rgb565_effect_fastpath(core, "row total unavailable"),
    };
    let stride: u32 = match read_generic(core, sp.wrapping_add(0x10)) {
        Ok(value) => value,
        Err(_) => return disable_inotia2_rgb565_effect_fastpath(core, "row stride unavailable"),
    };
    if total_rows == 0
        || total_rows > INOTIA2_RGB565_MAX_DIMENSION
        || current_row >= total_rows
    {
        return disable_inotia2_rgb565_effect_fastpath(core, "invalid row range");
    }

    let byte_len = (width as usize) * 2;
    if stride < byte_len as u32 {
        return disable_inotia2_rgb565_effect_fastpath(core, "stride smaller than row width");
    }
    let rows_remaining = (total_rows - current_row) as usize;
    let batch_bytes = match rows_remaining.checked_mul(byte_len) {
        Some(value) if value <= INOTIA2_RGB565_MAX_BATCH_BYTES => value,
        _ => return disable_inotia2_rgb565_effect_fastpath(core, "RGB565 batch exceeds host guard"),
    };

    // Pixel-format masks are initialized by the game's own graphics setup.
    let red_mask_ptr: u32 = match read_generic(core, got.wrapping_add(0x10e4)) {
        Ok(value) => value,
        Err(_) => return disable_inotia2_rgb565_effect_fastpath(core, "red-mask pointer unreadable"),
    };
    let green_mask_ptr: u32 = match read_generic(core, got.wrapping_add(0x10e0)) {
        Ok(value) => value,
        Err(_) => return disable_inotia2_rgb565_effect_fastpath(core, "green-mask pointer unreadable"),
    };
    let blue_mask_ptr: u32 = match read_generic(core, got.wrapping_add(0x10e8)) {
        Ok(value) => value,
        Err(_) => return disable_inotia2_rgb565_effect_fastpath(core, "blue-mask pointer unreadable"),
    };
    let red_mask: u32 = match read_generic(core, red_mask_ptr) {
        Ok(value) => value,
        Err(_) => return disable_inotia2_rgb565_effect_fastpath(core, "red-mask unreadable"),
    };
    let green_mask: u32 = match read_generic(core, green_mask_ptr) {
        Ok(value) => value,
        Err(_) => return disable_inotia2_rgb565_effect_fastpath(core, "green-mask unreadable"),
    };
    let blue_mask: u32 = match read_generic(core, blue_mask_ptr) {
        Ok(value) => value,
        Err(_) => return disable_inotia2_rgb565_effect_fastpath(core, "blue-mask unreadable"),
    };
    if red_mask != 0xf800 || green_mask != 0x07e0 || blue_mask != 0x001f {
        return disable_inotia2_rgb565_effect_fastpath(core, "pixel format is not RGB565");
    }

    let lut_ptr: u32 = match read_generic(core, got.wrapping_add(0x113c)) {
        Ok(value) if value != 0 => value,
        _ => return disable_inotia2_rgb565_effect_fastpath(core, "lookup-table pointer unavailable"),
    };

    let ref_r: u32 = match read_generic(core, sp.wrapping_add(0x58)) {
        Ok(value) => value,
        Err(_) => return disable_inotia2_rgb565_effect_fastpath(core, "reference red unavailable"),
    };
    let ref_g: u32 = match read_generic(core, sp.wrapping_add(0x54)) {
        Ok(value) => value,
        Err(_) => return disable_inotia2_rgb565_effect_fastpath(core, "reference green unavailable"),
    };
    let ref_b: u32 = match read_generic(core, sp.wrapping_add(0x50)) {
        Ok(value) => value,
        Err(_) => return disable_inotia2_rgb565_effect_fastpath(core, "reference blue unavailable"),
    };
    let ref_r_index = ((ref_r >> 3) & 31) as usize;
    let ref_g_index = ((ref_g >> 3) & 31) as usize;
    let ref_b_index = ((ref_b >> 3) & 31) as usize;

    let last_offset = match ((rows_remaining - 1) as u32).checked_mul(stride) {
        Some(value) => value,
        None => return disable_inotia2_rgb565_effect_fastpath(core, "row offset overflow"),
    };
    let last_row_ptr = match row_ptr.checked_add(last_offset) {
        Some(value) => value,
        None => return disable_inotia2_rgb565_effect_fastpath(core, "row pointer overflow"),
    };
    let last_row_end = match last_row_ptr.checked_add(byte_len as u32) {
        Some(value) => value,
        None => return disable_inotia2_rgb565_effect_fastpath(core, "final row end overflow"),
    };

    let mut state = shared.lock();
    if !state.lut_valid || state.lut_ptr != lut_ptr {
        if core.read_bytes(lut_ptr, &mut state.lut).is_err() {
            drop(state);
            return disable_inotia2_rgb565_effect_fastpath(core, "lookup table unreadable");
        }
        state.lut_ptr = lut_ptr;
        state.lut_valid = true;
        tracing::debug!(
            "[PHASE8_28_INOTIA2_RGB565_BATCH] cached LUT ptr={lut_ptr:#010x} bytes=1024"
        );
    }

    state.pixels.resize(batch_bytes, 0);
    for row_index in 0..rows_remaining {
        let offset = (row_index as u32) * stride; // guarded by last_offset above
        let address = row_ptr + offset;
        let begin = row_index * byte_len;
        let end = begin + byte_len;
        if core.read_bytes(address, &mut state.pixels[begin..end]).is_err() {
            drop(state);
            return disable_inotia2_rgb565_effect_fastpath(core, "RGB565 row unreadable");
        }
    }

    // One 1-KiB host-side copy per complete rectangle is deliberately cheap
    // and keeps the borrow structure simple; the expensive guest-memory LUT
    // read is still performed only when the pointer changes.
    let lut = state.lut.clone();
    let mut last_packed = regs.r0;
    for pixel_bytes in state.pixels[..batch_bytes].chunks_exact_mut(2) {
        let pixel = u16::from_le_bytes([pixel_bytes[0], pixel_bytes[1]]);
        let r_index = ((pixel >> 11) & 0x1f) as usize;
        let g_index = (((pixel >> 5) & 0x3f) >> 1) as usize;
        let b_index = (pixel & 0x1f) as usize;

        let r = lut[r_index * 32 + ref_r_index];
        let g = lut[g_index * 32 + ref_g_index];
        let b = lut[b_index * 32 + ref_b_index];
        let packed = (((r as u16) & 0xf8) << 8)
            | (((g as u16) & 0xfc) << 3)
            | ((b as u16) >> 3);
        pixel_bytes.copy_from_slice(&packed.to_le_bytes());
        last_packed = packed as u32;
    }

    // Guest rows can be padded, so write only the transformed clipped width.
    // A read failure above occurs before any writes; a write failure here is a
    // genuine mapped-memory violation and should propagate rather than replay
    // already-transformed rows through the guest fallback loop.
    for row_index in 0..rows_remaining {
        let address = row_ptr + (row_index as u32) * stride;
        let begin = row_index * byte_len;
        let end = begin + byte_len;
        core.write_bytes(address, &state.pixels[begin..end])?;
    }

    if !state.logged_first_batch {
        state.logged_first_batch = true;
        tracing::info!(
            "[PHASE8_28_INOTIA2_RGB565_BATCH] first batch accelerated width={width} rows={rows_remaining} stride={stride} bytes={batch_bytes}"
        );
    }
    drop(state);

    // Emulate the natural state after the final inner+outer iteration. The
    // original final-row path increments sp+0x30 to total_rows and branches
    // directly to 0x00123d4a without advancing r10 past the final row.
    write_generic(core, sp.wrapping_add(0x30), total_rows)?;
    regs.r0 = last_packed;
    regs.r5 = width;
    regs.r6 = last_row_end;
    regs.sl = last_row_ptr; // guest r10 at the final row
    core.restore_context(&regs);

    Ok(JumpTo(INOTIA2_RGB565_EFFECT_EPILOGUE_PC))
}

async fn handle_inotia2_rgb565_effect_svc(
    core: &mut ArmCore,
    shared: &mut Inotia2Rgb565EffectSharedState,
) -> Result<JumpTo> {
    handle_inotia2_rgb565_effect_batch(core, shared)
}

fn install_inotia2_rgb565_effect_fastpath(core: &mut ArmCore) -> Result<()> {
    let mut current = [0u8; 2];
    core.read_bytes(INOTIA2_RGB565_EFFECT_HOOK_ADDR, &mut current)?;
    if current != INOTIA2_RGB565_EFFECT_ORIGINAL {
        tracing::warn!(
            "[PHASE8_28_INOTIA2_RGB565_BATCH] install guard mismatch at {INOTIA2_RGB565_EFFECT_HOOK_ADDR:#010x}: got={current:02x?}; acceleration suppressed"
        );
        return Ok(());
    }

    let shared = Arc::new(spin::Mutex::new(Inotia2Rgb565EffectState {
        lut_ptr: 0,
        lut_valid: false,
        lut: [0u8; 32 * 32],
        pixels: Vec::new(),
        logged_first_batch: false,
    }));
    core.register_svc_handler(
        SVC_CATEGORY_INOTIA2_RGB565_EFFECT,
        handle_inotia2_rgb565_effect_svc,
        &shared,
    )?;
    core.write_bytes(
        INOTIA2_RGB565_EFFECT_HOOK_ADDR,
        &INOTIA2_RGB565_EFFECT_SVC,
    )?;
    tracing::info!(
        "[PHASE8_28_INOTIA2_RGB565_BATCH] installed hook={INOTIA2_RGB565_EFFECT_HOOK_PC:#010x}; all-rows RGB565 batch + LUT/buffer reuse enabled"
    );
    Ok(())
}


// Phase 8.26 — exact-title host acceleration for Inotia 2's LZMA1 resource
// decoder. The remaining black startup interval is not IndexedDB latency: the
// Phase 8.25 field log still shows repeated ~65.5M-instruction guest runs while
// static resources are expanded. Static analysis resolves guest 0x00125928 as
// Phase 8.27 — corrected Inotia 2 host LZMA wrapper.
//
// Exact disassembly of guest 0x00125928 plus the Phase 8.26 field failure
// resolves the actual private stream ABI:
//   r0 = pointer one byte *into* the packaged resource wrapper
//   r1 = remaining compressed byte length
//   r2 = caller-allocated output buffer
//   r3 = expected unpacked payload length
//
// Relative to r0, bytes 1..5 are the normal five-byte LZMA properties block,
// bytes 6..9 contain the same little-endian u32 unpacked length, bytes 10..13
// are title-private metadata, and raw LZMA payload begins at byte 14. Phase
// 8.26 incorrectly treated bytes 6..13 as the LZMA-Alone u64 length field,
// causing the runtime gate to restore the guest decoder every launch.
//
// Rebuild only the standard 13-byte LZMA-Alone header in host memory
// (properties + r3 as u64), append the original raw payload, decode it, and
// return exactly as the original wrapper would. Allocation, database rebuild,
// runtime table initialization, and callers remain guest-owned. Any guard or
// decoder failure restores the exact original PUSH instruction.
const SVC_CATEGORY_INOTIA2_LZMA: u32 = 0x82;
const INOTIA2_LZMA_HOOK_ADDR: u32 = 0x0012_5928;
const INOTIA2_LZMA_HOOK_PC: u32 = 0x0012_5929;
const INOTIA2_LZMA_ORIGINAL: [u8; 2] = [0xf0, 0xb5]; // push {r4,r5,r6,r7,lr}
const INOTIA2_LZMA_SVC: [u8; 2] = [0x82, 0xdf];
const INOTIA2_LZMA_MAX_COMPRESSED: usize = 16 * 1024 * 1024;
const INOTIA2_LZMA_MAX_OUTPUT: usize = 32 * 1024 * 1024;

static INOTIA2_LZMA_FIRST_USE: spin::Mutex<bool> = spin::Mutex::new(false);

fn disable_inotia2_lzma_fastpath(core: &mut ArmCore, reason: &str) -> Result<JumpTo> {
    core.write_bytes(INOTIA2_LZMA_HOOK_ADDR, &INOTIA2_LZMA_ORIGINAL)?;
    tracing::warn!(
        "[PHASE8_27_INOTIA2_LZMA_FASTPATH] runtime gate failed ({reason}); original guest decoder restored"
    );
    Ok(JumpTo(INOTIA2_LZMA_HOOK_PC))
}

async fn handle_inotia2_lzma_svc(core: &mut ArmCore, _: &mut ()) -> Result<JumpTo> {
    let mut regs = core.save_context();
    let compressed_ptr = regs.r0;
    let compressed_len = regs.r1 as usize;
    let output_ptr = regs.r2;
    let expected_len = regs.r3 as usize;

    if compressed_ptr == 0 || output_ptr == 0 {
        return disable_inotia2_lzma_fastpath(core, "null input/output pointer");
    }
    if compressed_len < 15 || compressed_len > INOTIA2_LZMA_MAX_COMPRESSED {
        return disable_inotia2_lzma_fastpath(core, "compressed length outside guard");
    }
    if expected_len == 0 || expected_len > INOTIA2_LZMA_MAX_OUTPUT {
        return disable_inotia2_lzma_fastpath(core, "unpacked length outside guard");
    }

    let mut compressed = vec![0u8; compressed_len];
    if core.read_bytes(compressed_ptr, &mut compressed).is_err() {
        return disable_inotia2_lzma_fastpath(core, "compressed guest buffer unreadable");
    }

    // r0 points at wrapper byte 1. The guest itself passes r0+1 to its
    // five-byte LZMA property parser, so the canonical property block here is
    // compressed[1..6].  The private u32 at compressed[6..10] is independently
    // read by guest helper 0x0012599c and becomes this wrapper's r3.
    let properties = compressed[1];
    if properties > 0xe0 {
        return disable_inotia2_lzma_fastpath(core, "invalid LZMA properties");
    }
    let dictionary_size = u32::from_le_bytes([
        compressed[2],
        compressed[3],
        compressed[4],
        compressed[5],
    ]) as usize;
    if dictionary_size == 0 || dictionary_size > INOTIA2_LZMA_MAX_OUTPUT {
        return disable_inotia2_lzma_fastpath(core, "dictionary outside host guard");
    }
    let private_declared_len = u32::from_le_bytes([
        compressed[6],
        compressed[7],
        compressed[8],
        compressed[9],
    ]) as usize;
    if private_declared_len != expected_len {
        return disable_inotia2_lzma_fastpath(core, "private u32/output length mismatch");
    }

    // lzma-rs consumes LZMA-Alone. Inotia 2 stores the same raw LZMA payload
    // but replaces the normal eight-byte output-size field with private
    // metadata. Synthesize only that header in temporary host memory:
    //
    //   [5-byte props from guest][u64 r3][raw payload from guest+14]
    //
    // This exact reconstruction was validated against eventdata.dat,
    // filetext.dat, i_mapfeature.dat, i_tile.dat and game.dat from this title.
    let payload_len = compressed_len - 14;
    let mut host_stream = Vec::with_capacity(13 + payload_len);
    host_stream.extend_from_slice(&compressed[1..6]);
    host_stream.extend_from_slice(&(expected_len as u64).to_le_bytes());
    host_stream.extend_from_slice(&compressed[14..]);

    let mut input = std::io::Cursor::new(&host_stream);
    let mut output = Vec::with_capacity(expected_len);
    if lzma_rs::lzma_decompress(&mut input, &mut output).is_err() {
        return disable_inotia2_lzma_fastpath(core, "host reconstructed LZMA decode failed");
    }
    if output.len() != expected_len {
        return disable_inotia2_lzma_fastpath(core, "decoded length differs from guest ABI");
    }
    if core.write_bytes(output_ptr, &output).is_err() {
        return disable_inotia2_lzma_fastpath(core, "decoded guest buffer unwritable");
    }

    regs.r0 = expected_len as u32;
    let return_pc = regs.lr;
    core.restore_context(&regs);

    let first_use = {
        let mut used = INOTIA2_LZMA_FIRST_USE.lock();
        let first = !*used;
        *used = true;
        first
    };
    if first_use {
        tracing::info!(
            "[PHASE8_27_INOTIA2_LZMA_FASTPATH] first decode accelerated compressed={} unpacked={} prefix={:#04x} props={:#04x} dict={}",
            compressed_len,
            expected_len,
            compressed[0],
            properties,
            dictionary_size
        );
    } else {
        tracing::trace!(
            "[PHASE8_27_INOTIA2_LZMA_FASTPATH] decode accelerated compressed={} unpacked={}",
            compressed_len,
            expected_len
        );
    }

    Ok(JumpTo(return_pc))
}

fn install_inotia2_lzma_fastpath(core: &mut ArmCore) -> Result<()> {
    let mut current = [0u8; 2];
    core.read_bytes(INOTIA2_LZMA_HOOK_ADDR, &mut current)?;
    if current != INOTIA2_LZMA_ORIGINAL {
        tracing::warn!(
            "[PHASE8_27_INOTIA2_LZMA_FASTPATH] install guard mismatch at {INOTIA2_LZMA_HOOK_ADDR:#010x}: got={current:02x?}; acceleration suppressed"
        );
        return Ok(());
    }

    *INOTIA2_LZMA_FIRST_USE.lock() = false;
    core.register_svc_handler(
        SVC_CATEGORY_INOTIA2_LZMA,
        handle_inotia2_lzma_svc,
        &(),
    )?;
    core.write_bytes(INOTIA2_LZMA_HOOK_ADDR, &INOTIA2_LZMA_SVC)?;
    tracing::info!(
        "[PHASE8_27_INOTIA2_LZMA_FASTPATH] installed hook={INOTIA2_LZMA_HOOK_PC:#010x}; private-header reconstruction guards enabled"
    );
    Ok(())
}

pub async fn load_native(
    core: &mut ArmCore,
    system: &mut System,
    jvm: &Jvm,
    filename: &str,
    data: &[u8],
    ptr_jvm_context: u32,
    ptr_jvm_exception_context: u32,
) -> Result<ExeInterfaceFunctions> {
    let bss_size = parse_bss_size(filename)?;

    // Phase 7.4A diagnostic experiment:
    //
    // KTF client.bin declares its static BSS size in the filename.  WIE used
    // to map exactly code/data + BSS.  Large late-generation titles such as
    // Inotia 2 declare >1 MiB of BSS and may expect the handset loader to
    // leave additional contiguous writable address space after that region
    // for the ARM-side C runtime heap (_sbrk/new/malloc).
    //
    // Keep the BSS value passed to the guest completely unchanged.  We only
    // map extra address-space headroom so this experiment cannot make the
    // relocation/bootstrap code believe its BSS is larger than declared.
    const LARGE_BSS_THRESHOLD: u32 = 512 * 1024;
    const DIAGNOSTIC_NATIVE_HEADROOM: usize = 8 * 1024 * 1024;
    let headroom = if bss_size >= LARGE_BSS_THRESHOLD {
        DIAGNOSTIC_NATIVE_HEADROOM
    } else {
        0
    };
    let nominal_map_size = data.len() + bss_size as usize;
    let mapped_size = nominal_map_size + headroom;

    tracing::info!(
        "[KTF_MEM] native image file={filename} base={IMAGE_BASE:#x} data={:#x} bss={bss_size:#x} nominal_end={:#x} headroom={:#x} mapped_end={:#x} global_allocator_base={:#x} global_allocator_size={:#x}",
        data.len(),
        IMAGE_BASE as usize + nominal_map_size,
        headroom,
        IMAGE_BASE as usize + mapped_size,
        0x4000_0000u32,
        0x1000_0000u32,
    );

    core.load(data, IMAGE_BASE, mapped_size)?;

    // Phase 8.13 — Inotia 2 KTF legacy certificate-validation bypass.
    //
    // PD007974 has already passed its access-level and executable-name gates
    // by this point. The next function at guest 0x0012ae44 validates the
    // carrier-era cert.c2s/tcert.c2s pair. The preserved archive has no valid
    // online companion certificate, and Phase 8.12 proved that aliasing
    // cert.c2s as tcert.c2s merely stalls inside this obsolete validator.
    //
    // The caller at 0x00176ab6 treats any nonzero return as success and
    // continues through the title's normal startup path. For this exact
    // AID/PID and known native-image length, replace only the validator entry
    // with `movs r0,#1; bx lr`. Guard the original bytes so a different game
    // revision is never patched by address alone.
    const INOTIA2_AID: &str = "010100D5";
    const INOTIA2_PID: &str = "PD007974";
    const INOTIA2_NATIVE_LEN: usize = 608_192;
    const INOTIA2_CERT_VALIDATOR: u32 = 0x0012_ae44;
    const INOTIA2_CERT_EXPECT: [u8; 4] = [0xf0, 0xb5, 0x57, 0x46];
    const INOTIA2_CERT_BYPASS: [u8; 4] = [0x01, 0x20, 0x70, 0x47];

    if system.aid() == INOTIA2_AID
        && system.pid() == INOTIA2_PID
        && data.len() == INOTIA2_NATIVE_LEN
    {
        let mut current = [0u8; 4];
        core.read_bytes(INOTIA2_CERT_VALIDATOR, &mut current)?;

        if current == INOTIA2_CERT_EXPECT {
            core.write_bytes(INOTIA2_CERT_VALIDATOR, &INOTIA2_CERT_BYPASS)?;
            tracing::info!(
                "[PHASE8_13_INOTIA2_CERT_BYPASS] validator={INOTIA2_CERT_VALIDATOR:#010x} expect={INOTIA2_CERT_EXPECT:02x?} -> return 1"
            );
        } else if current == INOTIA2_CERT_BYPASS {
            tracing::info!(
                "[PHASE8_13_INOTIA2_CERT_BYPASS] validator already patched at {INOTIA2_CERT_VALIDATOR:#010x}"
            );
        } else {
            tracing::warn!(
                "[PHASE8_13_INOTIA2_CERT_BYPASS] byte guard mismatch at {INOTIA2_CERT_VALIDATOR:#010x}: got={current:02x?}; patch suppressed"
            );
        }

        // Phase 8.18 — do not bypass the native installation/rebuild routine.
        //
        // Phase 8.17 NOPed the caller branch at 0x001780e2. Field testing
        // proved that routine also performs required in-memory resource-table
        // initialization: skipping it removes the progress bar, but the title
        // immediately falls back to its own "memory error" screen. Keep the
        // original control flow intact. The database layer now makes this
        // required pass cheap and non-destructive by buffering static install
        // records and committing each only once at close.

        // Phase 8.22 — suppress only the obsolete install/progress renderer.
        //
        // The required initializer at 0x00144f48 must still execute. Static
        // disassembly of this exact client separates its two calls to the
        // progress-window renderer (0x001449a0) from the resource/decompression
        // work. NOP only those two BL call sites. This removes the visible
        // "installation" bar while preserving every database open, cache
        // expansion, pointer-table update, and completion check that the title
        // needs to reach the main menu.
        const INOTIA2_PROGRESS_NOP: [u8; 4] = [0xc0, 0x46, 0xc0, 0x46];
        const INOTIA2_PROGRESS_CALLS: [(u32, [u8; 4], &str); 2] = [
            (0x0014_4f86, [0xff, 0xf7, 0x0b, 0xfd], "initial"),
            (0x0014_4fda, [0xff, 0xf7, 0xe1, 0xfc], "update"),
        ];
        for (address, expected, label) in INOTIA2_PROGRESS_CALLS {
            let mut current = [0u8; 4];
            core.read_bytes(address, &mut current)?;
            if current == expected {
                core.write_bytes(address, &INOTIA2_PROGRESS_NOP)?;
                tracing::info!(
                    "[PHASE8_22_INOTIA2_INSTALL_UI_SUPPRESS] site={label} address={address:#010x} progress renderer disabled"
                );
            } else if current == INOTIA2_PROGRESS_NOP {
                tracing::info!(
                    "[PHASE8_22_INOTIA2_INSTALL_UI_SUPPRESS] site={label} address={address:#010x} already patched"
                );
            } else {
                tracing::warn!(
                    "[PHASE8_22_INOTIA2_INSTALL_UI_SUPPRESS] site={label} address={address:#010x} guard mismatch got={current:02x?}; patch suppressed"
                );
            }
        }
    }

    // Phase 8.22 — Inotia 1 cash-shop response-state correction.
    //
    // The earlier Phase 8.17 experiment patched command 0 at 0x001171e4 to
    // jump straight to the command-1 request builder. Thumb disassembly now
    // shows why error 2009 survived every later parser bypass: the common packet
    // dispatcher itself consumes one response-state byte from every frame and
    // stores it through r10+0x470 before entering a command handler. Our old
    // command-0 frame omitted that byte and our old command-1 frame supplied 0,
    // so command 1 deterministically hit its early state==0 error-2009 branch at
    // 0x00117258, before 0x00117418/0x001174fe could matter. Do not patch command
    // 0 here anymore. The offline transport now supplies the original state==1
    // field in both frames and lets the title follow its own dispatcher paths.
    const INOTIA1_AID: &str = "010100D3";
    const INOTIA1_PID: &str = "PD005362";
    const INOTIA1_NATIVE_LEN: usize = 431_008;

    if system.aid() == INOTIA1_AID
        && system.pid() == INOTIA1_PID
        && data.len() == INOTIA1_NATIVE_LEN
    {
        // Phase 8.20 — command-1 offline-response validation bypass.
        //
        // Phase 8.19 proved that the client completely consumes our structurally
        // correct command-1 response, then reaches the common network-error
        // cleanup before issuing another request. Static analysis resolves the
        // first post-parse guard at 0x00117418: after calling the title's legacy
        // response validator at 0x0011d108, a conditional BNE advances to the
        // normal command-1 processing path while validator==0 falls directly
        // into error 2009. The historical carrier/server integrity context no
        // longer exists, so for this exact binary only, make that conditional
        // branch unconditional while preserving the validator call and all of
        // its side effects. This does not alter inventory/save writes or any
        // purchase result; later cash-shop commands are still captured before
        // we synthesize them.
        const INOTIA1_CMD1_VALID_BRANCH: u32 = 0x0011_7418;
        const INOTIA1_CMD1_VALID_EXPECT: [u8; 2] = [0x00, 0xd1]; // bne +0 -> 0x11741c
        const INOTIA1_CMD1_VALID_BYPASS: [u8; 2] = [0x00, 0xe0]; // b   +0 -> 0x11741c

        let mut cmd1_valid_branch = [0u8; 2];
        core.read_bytes(INOTIA1_CMD1_VALID_BRANCH, &mut cmd1_valid_branch)?;
        if cmd1_valid_branch == INOTIA1_CMD1_VALID_EXPECT {
            core.write_bytes(INOTIA1_CMD1_VALID_BRANCH, &INOTIA1_CMD1_VALID_BYPASS)?;
            tracing::info!(
                "[PHASE8_20_INOTIA1_CASH_CMD1_VALIDATION_BYPASS] branch={INOTIA1_CMD1_VALID_BRANCH:#010x} legacy validator failure -> continue original command-1 success path"
            );
        } else if cmd1_valid_branch == INOTIA1_CMD1_VALID_BYPASS {
            tracing::info!(
                "[PHASE8_20_INOTIA1_CASH_CMD1_VALIDATION_BYPASS] branch already patched at {INOTIA1_CMD1_VALID_BRANCH:#010x}"
            );
        } else {
            tracing::warn!(
                "[PHASE8_20_INOTIA1_CASH_CMD1_VALIDATION_BYPASS] byte guard mismatch at {INOTIA1_CMD1_VALID_BRANCH:#010x}: got={cmd1_valid_branch:02x?}; patch suppressed"
            );
        }

        // Phase 8.21 — second command-1 data/integrity gate.
        //
        // Phase 8.20 proved that bypassing the first legacy validator was not
        // sufficient. The real client fully consumes the 27-byte local frame,
        // then records cash error 2009/state 5 and closes the socket. Static
        // analysis of the same command-1 handler resolves the next direct
        // error-2009 branch at 0x001174fe: helper 0x0011d158 advances the
        // packet cursor, runs the extinct carrier/session integrity check, and
        // returns zero when that historical context is unavailable. Preserve
        // the helper call and its cursor/state side effects, but suppress only
        // the `beq error_2009` branch for this exact binary. The following
        // original code builds the next protocol request, which our offline
        // bridge will capture before any catalog/purchase response is invented.
        const INOTIA1_CMD1_DATA_VALID_BRANCH: u32 = 0x0011_74fe;
        const INOTIA1_CMD1_DATA_VALID_EXPECT: [u8; 2] = [0x35, 0xd0]; // beq 0x11756c
        const INOTIA1_CMD1_DATA_VALID_BYPASS: [u8; 2] = [0xc0, 0x46]; // Thumb NOP

        let mut cmd1_data_valid_branch = [0u8; 2];
        core.read_bytes(INOTIA1_CMD1_DATA_VALID_BRANCH, &mut cmd1_data_valid_branch)?;
        if cmd1_data_valid_branch == INOTIA1_CMD1_DATA_VALID_EXPECT {
            core.write_bytes(
                INOTIA1_CMD1_DATA_VALID_BRANCH,
                &INOTIA1_CMD1_DATA_VALID_BYPASS,
            )?;
            tracing::info!(
                "[PHASE8_21_INOTIA1_CASH_CMD1_DATA_VALIDATION_BYPASS] branch={INOTIA1_CMD1_DATA_VALID_BRANCH:#010x} error-2009 data validator -> continue original request builder"
            );
        } else if cmd1_data_valid_branch == INOTIA1_CMD1_DATA_VALID_BYPASS {
            tracing::info!(
                "[PHASE8_21_INOTIA1_CASH_CMD1_DATA_VALIDATION_BYPASS] branch already patched at {INOTIA1_CMD1_DATA_VALID_BRANCH:#010x}"
            );
        } else {
            tracing::warn!(
                "[PHASE8_21_INOTIA1_CASH_CMD1_DATA_VALIDATION_BYPASS] byte guard mismatch at {INOTIA1_CMD1_DATA_VALID_BRANCH:#010x}: got={cmd1_data_valid_branch:02x?}; patch suppressed"
            );
        }


        // Phase 8.28 — restore the client's network-only item-use path offline.
        //
        // Static analysis of this exact 431,008-byte client resolves guest
        // 0x0015032e as the network-state gate for special action/type 13. The
        // original code requires global network state == 2; otherwise it stores
        // the client's sole literal cash/network error 2001 and returns through
        // the generic failure UI. This is the path hit by 자원 교환권, whose
        // original description says it exchanges for 10 network resources but
        // whose original restriction string says it is usable only while
        // connected to the network.
        //
        // Do not fake network mode globally. For this exact title only, change
        // the one BEQ to an unconditional branch to the *existing* valid-use
        // continuation. The original item-consumption/resource-grant code then
        // remains responsible for inventory and save state. The same 2001 gate
        // is also the strongest candidate for the residual first-entry cash-shop
        // error seen after the catalog already arrived, so this may remove that
        // popup without patching UI state directly.
        const INOTIA1_NETWORK_ITEM_USE_BRANCH: u32 = 0x0015_032e;
        const INOTIA1_NETWORK_ITEM_USE_EXPECT: [u8; 2] = [0x06, 0xd0]; // beq 0x15033e
        const INOTIA1_NETWORK_ITEM_USE_BYPASS: [u8; 2] = [0x06, 0xe0]; // b   0x15033e

        let mut network_item_use_branch = [0u8; 2];
        core.read_bytes(INOTIA1_NETWORK_ITEM_USE_BRANCH, &mut network_item_use_branch)?;
        if network_item_use_branch == INOTIA1_NETWORK_ITEM_USE_EXPECT {
            core.write_bytes(
                INOTIA1_NETWORK_ITEM_USE_BRANCH,
                &INOTIA1_NETWORK_ITEM_USE_BYPASS,
            )?;
            tracing::info!(
                "[PHASE8_28_INOTIA1_NETWORK_USE_GATE] branch={INOTIA1_NETWORK_ITEM_USE_BRANCH:#010x} network-state==2 requirement bypassed; original item-use continuation preserved"
            );
        } else if network_item_use_branch == INOTIA1_NETWORK_ITEM_USE_BYPASS {
            tracing::info!(
                "[PHASE8_28_INOTIA1_NETWORK_USE_GATE] branch already patched at {INOTIA1_NETWORK_ITEM_USE_BRANCH:#010x}"
            );
        } else {
            tracing::warn!(
                "[PHASE8_28_INOTIA1_NETWORK_USE_GATE] byte guard mismatch at {INOTIA1_NETWORK_ITEM_USE_BRANCH:#010x}: got={network_item_use_branch:02x?}; patch suppressed"
            );
        }


        // Phase 8.29 — enable the second network-only consumable gate.
        //
        // The Phase 8.28 state==2 bypass reaches the original valid-use block,
        // but that block contains a second BLS back to the same sole error-2001
        // handler for the contiguous network-special item-ID range 0xF4..0xFE.
        // 축복받은 용사의 인장 and the other network-only consumables are in
        // this range, so single-player still showed the original "network mode
        // only" message. Preserve all original item-use/grant logic and NOP only
        // this exact title-specific rejection branch.
        const INOTIA1_NETWORK_SPECIAL_ID_BRANCH: u32 = 0x0015_034a;
        const INOTIA1_NETWORK_SPECIAL_ID_EXPECT: [u8; 2] = [0xf1, 0xd9]; // bls 0x150330 (error 2001)
        const INOTIA1_NETWORK_SPECIAL_ID_BYPASS: [u8; 2] = [0xc0, 0x46]; // Thumb NOP -> original use path

        let mut network_special_id_branch = [0u8; 2];
        core.read_bytes(INOTIA1_NETWORK_SPECIAL_ID_BRANCH, &mut network_special_id_branch)?;
        if network_special_id_branch == INOTIA1_NETWORK_SPECIAL_ID_EXPECT {
            core.write_bytes(
                INOTIA1_NETWORK_SPECIAL_ID_BRANCH,
                &INOTIA1_NETWORK_SPECIAL_ID_BYPASS,
            )?;
            tracing::info!(
                "[PHASE8_29_INOTIA1_NETWORK_SPECIAL_USE_GATE] branch={INOTIA1_NETWORK_SPECIAL_ID_BRANCH:#010x} network-special ID rejection removed; original single-player use continuation preserved"
            );
        } else if network_special_id_branch == INOTIA1_NETWORK_SPECIAL_ID_BYPASS {
            tracing::info!(
                "[PHASE8_29_INOTIA1_NETWORK_SPECIAL_USE_GATE] branch already patched at {INOTIA1_NETWORK_SPECIAL_ID_BRANCH:#010x}"
            );
        } else {
            tracing::warn!(
                "[PHASE8_29_INOTIA1_NETWORK_SPECIAL_USE_GATE] byte guard mismatch at {INOTIA1_NETWORK_SPECIAL_ID_BRANCH:#010x}: got={network_special_id_branch:02x?}; patch suppressed"
            );
        }


        // Phase 8.30 — enable the remaining blessed-seal network-mode gate.
        //
        // Phase 8.29 removed the item-dispatch range rejection at 0x0015034a,
        // but the field test still showed 축복받은 용사의 인장 rejected before
        // any network packet was emitted. A second exact network-state check is
        // reached through the item's property/type validator at guest 0x001485b6:
        // after the current item resolves to type 0xBA, the client requires
        // global network state == 2 or routes to the same network-only failure
        // UI. Preserve the item/type check and every use-effect call; change
        // only this BEQ to the existing valid continuation for this exact
        // Inotia 1 binary.
        const INOTIA1_BLESSED_SEAL_NETWORK_BRANCH: u32 = 0x0014_85b6;
        const INOTIA1_BLESSED_SEAL_NETWORK_EXPECT: [u8; 2] = [0x0a, 0xd0]; // beq 0x1485ce
        const INOTIA1_BLESSED_SEAL_NETWORK_BYPASS: [u8; 2] = [0x0a, 0xe0]; // b   0x1485ce

        let mut blessed_seal_network_branch = [0u8; 2];
        core.read_bytes(
            INOTIA1_BLESSED_SEAL_NETWORK_BRANCH,
            &mut blessed_seal_network_branch,
        )?;
        if blessed_seal_network_branch == INOTIA1_BLESSED_SEAL_NETWORK_EXPECT {
            core.write_bytes(
                INOTIA1_BLESSED_SEAL_NETWORK_BRANCH,
                &INOTIA1_BLESSED_SEAL_NETWORK_BYPASS,
            )?;
            tracing::info!(
                "[PHASE8_30_INOTIA1_BLESSED_SEAL_USE_GATE] branch={INOTIA1_BLESSED_SEAL_NETWORK_BRANCH:#010x} type-0xBA network-state requirement bypassed; original item-use path preserved"
            );
        } else if blessed_seal_network_branch == INOTIA1_BLESSED_SEAL_NETWORK_BYPASS {
            tracing::info!(
                "[PHASE8_30_INOTIA1_BLESSED_SEAL_USE_GATE] branch already patched at {INOTIA1_BLESSED_SEAL_NETWORK_BRANCH:#010x}"
            );
        } else {
            tracing::warn!(
                "[PHASE8_30_INOTIA1_BLESSED_SEAL_USE_GATE] byte guard mismatch at {INOTIA1_BLESSED_SEAL_NETWORK_BRANCH:#010x}: got={blessed_seal_network_branch:02x?}; patch suppressed"
            );
        }
    }

    // Patterns target instruction encodings, which the guest self-rebase at
    // IMAGE_BASE+1 doesn't rewrite — so installing here is sound and skips a
    // re-scan after relocation. Hash-matched entries take priority over
    // hash-less generic ones; only one entry is installed because each install
    // claims fresh SVC categories from a fixed base and they would collide.
    //
    // The scan range covers the whole loaded image because KTF binaries don't
    // expose a code/metadata boundary at this point. Safety relies on the
    // patterns being long enough (and `{exit_b}` strict enough) that a
    // metadata-region collision is implausible; tighten patterns rather than
    // narrow the range if that ever becomes false.
    wie_core_arm::install_binary_patches(core, data, &[(IMAGE_BASE, data.len() as u32)])?;

    if system.aid() == INOTIA2_AID
        && system.pid() == INOTIA2_PID
        && data.len() == INOTIA2_NATIVE_LEN
    {
        install_inotia2_rgb565_effect_fastpath(core)?;
        install_inotia2_lzma_fastpath(core)?;
    }

    register_wipic_svc_handler(core, system, jvm)?;
    register_init_svc_handler(core, jvm)?;

    tracing::debug!("Loaded at {IMAGE_BASE:#x}, size {:#x}, bss {bss_size:#x}", data.len());

    let wipi_exe = core.run_function(IMAGE_BASE + 1, &[bss_size]).await?;
    tracing::debug!("Got wipi_exe {wipi_exe:#x}");

    let ptr_param_0 = Allocator::alloc(core, size_of::<InitParam0>() as u32)?;
    write_generic(core, ptr_param_0, InitParam0 { unk: 0 })?;

    let ptr_param_1 = Allocator::alloc(core, size_of::<InitParam1>() as u32)?;
    write_generic(core, ptr_param_1, InitParam1 { ptr_jvm_exception_context })?;

    let param_3 = InitParam3 {
        unk1: 0,
        unk2: 0,
        unk3: 0,
        unk4: 0,
        boolean: b'Z' as u32,
        char: b'C' as u32,
        float: b'F' as u32,
        double: b'D' as u32,
        byte: b'B' as u32,
        short: b'S' as u32,
        int: b'I' as u32,
        long: b'J' as u32,
    };

    let ptr_param_3 = Allocator::alloc(core, size_of::<InitParam3>() as u32)?;
    write_generic(core, ptr_param_3, param_3)?;

    let param_4 = InitParam4 {
        fn_get_interface: core.make_svc_stub(SVC_CATEGORY_INIT, InitSvcId::GetInterface)?,
        fn_java_throw: core.make_svc_stub(SVC_CATEGORY_INIT, InitSvcId::JavaThrow)?,
        unk1: 0,
        unk2: 0,
        fn_java_check_type: core.make_svc_stub(SVC_CATEGORY_INIT, InitSvcId::JavaCheckType)?,
        fn_java_new: core.make_svc_stub(SVC_CATEGORY_INIT, InitSvcId::JavaNew)?,
        fn_java_array_new: core.make_svc_stub(SVC_CATEGORY_INIT, InitSvcId::JavaArrayNew)?,
        unk6: 0,
        fn_java_class_load: core.make_svc_stub(SVC_CATEGORY_INIT, InitSvcId::JavaClassLoad)?,
        unk7: 0,
        unk8: 0,
        fn_alloc: core.make_svc_stub(SVC_CATEGORY_INIT, InitSvcId::Alloc)?,
    };

    let ptr_param_4 = Allocator::alloc(core, size_of::<InitParam4>() as u32)?;
    write_generic(core, ptr_param_4, param_4)?;

    let wipi_exe: WipiExe = read_generic(core, wipi_exe)?;
    let exe_interface: ExeInterface = read_generic(core, wipi_exe.ptr_exe_interface)?;
    let exe_interface_functions: ExeInterfaceFunctions = read_generic(core, exe_interface.ptr_functions)?;

    tracing::info!(
        "[KTF_INIT] exe_interface_ptr={:#x} functions_ptr={:#x} fn_init={:#x}",
        wipi_exe.ptr_exe_interface,
        exe_interface.ptr_functions,
        exe_interface_functions.fn_init,
    );
    tracing::info!(
        "[KTF_INIT] params p0={ptr_param_0:#x} p1={ptr_param_1:#x} jvm={ptr_jvm_context:#x} p3={ptr_param_3:#x} p4={ptr_param_4:#x}"
    );
    tracing::debug!("Call init at {:#x}", exe_interface_functions.fn_init);
    let result = core
        .run_function::<u32>(
            exe_interface_functions.fn_init,
            &[ptr_param_0, ptr_param_1, ptr_jvm_context, ptr_param_3, ptr_param_4],
        )
        .await?;

    if result != 0 {
        return Err(WieError::FatalError(format!("Init failed with code {result:#x}")));
    }

    // call init
    let result = core.run_function::<u32>(wipi_exe.fn_init, &[]).await?;
    if result != 0 {
        return Err(WieError::FatalError(format!("wipi init failed with code {result:#x}")));
    }

    Ok(exe_interface_functions)
}

async fn get_interface(core: &mut ArmCore, ptr_name: u32) -> Result<u32> {
    let (pc, lr) = core.read_pc_lr()?;
    let caller = lr & !1;
    let name = String::from_utf8(read_null_terminated_string_bytes(core, ptr_name)?).unwrap();

    tracing::info!(
        "[KTF_IFACE] request={name} svc_pc={pc:#x} caller_lr={lr:#x} caller={caller:#x}"
    );

    // Capture a small word window around the return address.  This does not
    // disassemble the code on-device, but combined with wie_ktf_dump it lets
    // us line the runtime trace up with the relocated ARM image precisely.
    let window_start = caller.saturating_sub(16) & !3;
    let mut words = [0u32; 8];
    let mut words_ok = true;
    for (index, word) in words.iter_mut().enumerate() {
        match read_generic::<u32, _>(core, window_start + (index as u32 * 4)) {
            Ok(value) => *word = value,
            Err(_) => {
                words_ok = false;
                break;
            }
        }
    }
    if words_ok {
        tracing::info!(
            "[KTF_IFACE] caller_words base={window_start:#x} [{:#010x},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x}]",
            words[0], words[1], words[2], words[3], words[4], words[5], words[6], words[7]
        );
    } else {
        tracing::info!("[KTF_IFACE] caller_words unavailable base={window_start:#x}");
    }

    let result = match name.as_str() {
        "WIPIC_knlInterface" => get_wipic_knl_interface(core),
        "WIPI_JBInterface" => get_wipi_jb_interface(core),
        "WIPICX_incMemInterface" => get_inc_mem_interface(core),
        _ => {
            tracing::warn!("Unknown {name}");
            Ok(0)
        }
    }?;

    tracing::info!(
        "[KTF_IFACE] response={name} ptr={result:#x} caller={caller:#x}"
    );
    Ok(result)
}


/// KTF extension used by the ARM-side C/C++ runtime when its static allocator
/// needs another arena.  Several late WIPI 1.2 games (including Inotia 2)
/// refuse to enable new/malloc/free when this interface is absent.
///
/// The extension is a one-entry interface table.  The guest calls the entry
/// with a requested byte count and receives a writable guest-memory address.
fn get_inc_mem_interface(core: &mut ArmCore) -> Result<u32> {
    // The late KTF SDK extension is not publicly documented. Phase 7.2 proved
    // that returning a single function pointer at table[0] is insufficient:
    // Inotia 2 accepts the interface pointer but never invokes slot 0.
    //
    // Expose eight independently traced entries so the guest can use the slot
    // layout expected by its handset SDK. Each entry reaches the same guarded
    // allocator heuristic, but logs its table index for ABI discovery.
    const SLOT_COUNT: usize = 8;
    let address = Allocator::alloc(core, (SLOT_COUNT * size_of::<u32>()) as u32)?;

    for slot in 0..SLOT_COUNT {
        let id = match slot {
            0 => InitSvcId::IncMem0,
            1 => InitSvcId::IncMem1,
            2 => InitSvcId::IncMem2,
            3 => InitSvcId::IncMem3,
            4 => InitSvcId::IncMem4,
            5 => InitSvcId::IncMem5,
            6 => InitSvcId::IncMem6,
            7 => InitSvcId::IncMem7,
            _ => unreachable!(),
        };
        let stub = core.make_svc_stub(SVC_CATEGORY_INIT, id)?;
        write_generic(core, address + (slot as u32 * 4), stub)?;
        tracing::info!("WIPICX_incMemInterface slot={slot} fn={stub:#x}");
    }

    tracing::info!("WIPICX_incMemInterface multi-slot table={address:#x} slots={SLOT_COUNT}");
    Ok(address)
}

async fn inc_mem_slot(core: &mut ArmCore, slot: u32) -> Result<u32> {
    let a0 = core.read_param(0)?;
    let a1 = core.read_param(1)?;
    let a2 = core.read_param(2)?;
    let a3 = core.read_param(3)?;

    tracing::info!(
        "WIPICX_incMem slot={slot} called args=[{a0:#x},{a1:#x},{a2:#x},{a3:#x}]"
    );

    // A real allocation size should be nonzero and reasonably small. Guest
    // pointers live around 0x48xxxxxx, so rejecting very large values also
    // makes free/destroy-style methods harmless while we identify this ABI.
    const MIN_REQUEST: u32 = 16;
    const MAX_REQUEST: u32 = 32 * 1024 * 1024;
    let args = [a0, a1, a2, a3];
    let requested = args
        .into_iter()
        .find(|value| *value >= MIN_REQUEST && *value <= MAX_REQUEST);

    let Some(requested) = requested else {
        tracing::info!("WIPICX_incMem slot={slot} no allocation-sized argument; returning 0");
        return Ok(0);
    };

    let requested = requested.next_multiple_of(16);
    match Allocator::alloc(core, requested) {
        Ok(address) => {
            tracing::info!("WIPICX_incMem slot={slot} request={requested:#x} -> {address:#x}");
            Ok(address)
        }
        Err(error) => {
            tracing::warn!("WIPICX_incMem slot={slot} allocation failed request={requested:#x}: {error:?}");
            Ok(0)
        }
    }
}

async fn alloc(core: &mut ArmCore, _: &mut (), a0: u32) -> Result<u32> {
    tracing::trace!("alloc({a0})");

    Allocator::alloc(core, a0)
}
