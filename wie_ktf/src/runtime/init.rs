use alloc::{format, string::String, vec, vec::Vec};
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

// Phase 8.25 — exact-title host acceleration for Inotia 2's dominant RGB565
// software effect row. Phase 8.23 profiling tied the largest 300-664 ms frame
// gaps to the tight guest loop at 0x00123f2a..0x00123f82. The loop unpacks one
// RGB565 pixel, applies the same 32x32 byte lookup table independently to R/G/B,
// repacks the pixel, and repeats for every pixel in a clipped row. Interpreting
// that sequence millions of times is unnecessary: replace only the loop entry
// with a private SVC and reproduce one complete row in Rust using two bulk guest
// memory transfers.
//
// The hook is installed only for exact AID/PID/native length + original bytes.
// At runtime it additionally verifies the title's active pixel masks are exactly
// RGB565 (F800/07E0/001F). If that invariant ever fails, the original instruction
// is restored and execution resumes at the unmodified guest loop for the rest of
// the launch.
const SVC_CATEGORY_INOTIA2_RGB565_EFFECT: u32 = 0x81;
const INOTIA2_RGB565_EFFECT_HOOK_ADDR: u32 = 0x0012_3f2a;
const INOTIA2_RGB565_EFFECT_HOOK_PC: u32 = 0x0012_3f2b;
const INOTIA2_RGB565_EFFECT_EXIT_PC: u32 = 0x0012_3f85;
const INOTIA2_RGB565_EFFECT_ORIGINAL: [u8; 2] = [0x02, 0x99]; // ldr r1,[sp,#8]
const INOTIA2_RGB565_EFFECT_SVC: [u8; 2] = [0x81, 0xdf];

fn disable_inotia2_rgb565_effect_fastpath(core: &mut ArmCore, reason: &str) -> Result<JumpTo> {
    core.write_bytes(
        INOTIA2_RGB565_EFFECT_HOOK_ADDR,
        &INOTIA2_RGB565_EFFECT_ORIGINAL,
    )?;
    tracing::warn!(
        "[PHASE8_25_INOTIA2_RGB565_FASTPATH] runtime gate failed ({reason}); original guest loop restored"
    );
    Ok(JumpTo(INOTIA2_RGB565_EFFECT_HOOK_PC))
}

async fn handle_inotia2_rgb565_effect_svc(
    core: &mut ArmCore,
    _: &mut (),
) -> Result<JumpTo> {
    let mut regs = core.save_context();
    let got = regs.r7;
    let sp = regs.sp;
    let row_ptr = regs.r6;
    let width = regs.r8;

    // The loop is only valid for a positive clipped width. Keep a conservative
    // sanity cap so an unexpected call state can never request a huge host
    // allocation; fallback restores the original code instead.
    if width == 0 || width > 4096 {
        return disable_inotia2_rgb565_effect_fastpath(core, "invalid clipped width");
    }

    // Pixel-format masks are initialized by the game's own graphics setup.
    // GOT entries 0x10e4/0x10e0/0x10e8 point to the active R/G/B mask cells.
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

    // The hot loop loads the 32x32 transform table directly from GOT+0x113c.
    let lut_ptr: u32 = match read_generic(core, got.wrapping_add(0x113c)) {
        Ok(value) if value != 0 => value,
        _ => return disable_inotia2_rgb565_effect_fastpath(core, "lookup-table pointer unavailable"),
    };
    let mut lut = [0u8; 32 * 32];
    if core.read_bytes(lut_ptr, &mut lut).is_err() {
        return disable_inotia2_rgb565_effect_fastpath(core, "lookup table unreadable");
    }

    // The row's reference color was unpacked once immediately before entering
    // the loop into sp+0x58 (R), sp+0x54 (G), sp+0x50 (B). Only the top five
    // bits are used as LUT indices, exactly matching the original ASRS #3.
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

    let byte_len = (width as usize) * 2;
    let mut row = vec![0u8; byte_len];
    if core.read_bytes(row_ptr, &mut row).is_err() {
        return disable_inotia2_rgb565_effect_fastpath(core, "RGB565 row unreadable");
    }

    let mut last_packed = regs.r0;
    for pixel_bytes in row.chunks_exact_mut(2) {
        let pixel = u16::from_le_bytes([pixel_bytes[0], pixel_bytes[1]]);

        // RGB565 unpack -> the same 5-bit LUT indices produced by the title's
        // specialized unpacker followed by ASRS #3. Green has six source bits,
        // so its lower bit is intentionally discarded by the >>1.
        let r_index = ((pixel >> 11) & 0x1f) as usize;
        let g_index = (((pixel >> 5) & 0x3f) >> 1) as usize;
        let b_index = (pixel & 0x1f) as usize;

        let r = lut[r_index * 32 + ref_r_index];
        let g = lut[g_index * 32 + ref_g_index];
        let b = lut[b_index * 32 + ref_b_index];

        // Exact RGB565 packing convention used by the title's specialized
        // packer: high 5 R bits, high 6 G bits, high 5 B bits.
        let packed = (((r as u16) & 0xf8) << 8)
            | (((g as u16) & 0xfc) << 3)
            | ((b as u16) >> 3);
        pixel_bytes.copy_from_slice(&packed.to_le_bytes());
        last_packed = packed as u32;
    }

    if core.write_bytes(row_ptr, &row).is_err() {
        return disable_inotia2_rgb565_effect_fastpath(core, "RGB565 row unwritable");
    }

    // Reproduce the register state at the natural exit of the guest loop. The
    // following instruction at 0x00123f84 begins the row/height bookkeeping.
    regs.r0 = last_packed;
    regs.r5 = width;
    regs.r6 = row_ptr.wrapping_add(width.wrapping_mul(2));
    core.restore_context(&regs);

    tracing::trace!(
        "[PHASE8_25_INOTIA2_RGB565_FASTPATH] row={row_ptr:#010x} width={width} accelerated"
    );
    Ok(JumpTo(INOTIA2_RGB565_EFFECT_EXIT_PC))
}

fn install_inotia2_rgb565_effect_fastpath(core: &mut ArmCore) -> Result<()> {
    let mut current = [0u8; 2];
    core.read_bytes(INOTIA2_RGB565_EFFECT_HOOK_ADDR, &mut current)?;
    if current != INOTIA2_RGB565_EFFECT_ORIGINAL {
        tracing::warn!(
            "[PHASE8_25_INOTIA2_RGB565_FASTPATH] install guard mismatch at {INOTIA2_RGB565_EFFECT_HOOK_ADDR:#010x}: got={current:02x?}; acceleration suppressed"
        );
        return Ok(());
    }

    // Register before writing the SVC opcode. No guest thread is running yet,
    // but this order also makes the install atomic from the emulator's view.
    core.register_svc_handler(
        SVC_CATEGORY_INOTIA2_RGB565_EFFECT,
        handle_inotia2_rgb565_effect_svc,
        &(),
    )?;
    core.write_bytes(
        INOTIA2_RGB565_EFFECT_HOOK_ADDR,
        &INOTIA2_RGB565_EFFECT_SVC,
    )?;
    tracing::info!(
        "[PHASE8_25_INOTIA2_RGB565_FASTPATH] installed hook={INOTIA2_RGB565_EFFECT_HOOK_PC:#010x} exit={INOTIA2_RGB565_EFFECT_EXIT_PC:#010x}; runtime RGB565 mask guard enabled"
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
