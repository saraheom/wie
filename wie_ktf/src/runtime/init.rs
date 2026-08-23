use alloc::{format, string::String};
use core::mem::size_of;
use jvm::Jvm;

use wie_backend::System;
use wie_core_arm::{Allocator, ArmCore, EmulatedFunction, ResultWriter, SvcId};
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
    }

    // Phase 8.17 — Inotia 1 offline cash-shop bootstrap.
    //
    // The Phase 8.16 server-first frame [00 03 00] is accepted and dispatches
    // command 0, but the preserved client is in the wrong carrier-network
    // state and takes its local "error occurred" branch before transmitting a
    // request.  In the original command-0 handler, the state==1 branch at
    // 0x00117234 calls the title's own request builder as `send(1, 0)`.
    // Redirect only the command-0 entry to that existing branch so the client
    // generates its authentic next protocol packet; the offline socket layer
    // can then capture it without contacting the extinct service.
    const INOTIA1_AID: &str = "010100D3";
    const INOTIA1_PID: &str = "PD005362";
    const INOTIA1_NATIVE_LEN: usize = 431_008;
    const INOTIA1_CASH_CMD0_HANDLER: u32 = 0x0011_71e4;
    const INOTIA1_CASH_CMD0_EXPECT: [u8; 2] = [0xc3, 0x19];
    // Thumb `b 0x00117234` from 0x001171e4 (PC=0x001171e8).
    const INOTIA1_CASH_CMD0_BOOTSTRAP: [u8; 2] = [0x26, 0xe0];

    if system.aid() == INOTIA1_AID
        && system.pid() == INOTIA1_PID
        && data.len() == INOTIA1_NATIVE_LEN
    {
        let mut cash_cmd0 = [0u8; 2];
        core.read_bytes(INOTIA1_CASH_CMD0_HANDLER, &mut cash_cmd0)?;
        if cash_cmd0 == INOTIA1_CASH_CMD0_EXPECT {
            core.write_bytes(INOTIA1_CASH_CMD0_HANDLER, &INOTIA1_CASH_CMD0_BOOTSTRAP)?;
            tracing::info!(
                "[PHASE8_17_INOTIA1_CASH_CMD0_BOOTSTRAP] handler={INOTIA1_CASH_CMD0_HANDLER:#010x} -> force original command-1 request builder"
            );
        } else if cash_cmd0 == INOTIA1_CASH_CMD0_BOOTSTRAP {
            tracing::info!(
                "[PHASE8_17_INOTIA1_CASH_CMD0_BOOTSTRAP] handler already patched at {INOTIA1_CASH_CMD0_HANDLER:#010x}"
            );
        } else {
            tracing::warn!(
                "[PHASE8_17_INOTIA1_CASH_CMD0_BOOTSTRAP] byte guard mismatch at {INOTIA1_CASH_CMD0_HANDLER:#010x}: got={cash_cmd0:02x?}; patch suppressed"
            );
        }

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
