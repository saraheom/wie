use alloc::{boxed::Box, vec, vec::Vec};
use core::sync::atomic::{AtomicBool, Ordering};

use spin::Mutex;
use wipi_types::wipic::WIPICWord;

use wie_util::{Result, WieError};

use crate::{WIPICResult, context::WIPICContext, method::MethodBody};

// Phase 8.12 — Inotia 1 KTF offline network bridge.
//
// Inotia 1 (AID 010100D3 / PID PD005362) still contains its original network
// shop client, but the generic WIE MC_netConnect stub intentionally reports
// M_E_ERROR through the callback.  That prevents the title from reaching any
// of its socket/protocol code and makes the cash-shop option fail immediately.
//
// For this exact legacy title only, provide an offline transport shim.  It does
// *not* contact the historical Com2uS server.  Instead it:
//   * reports bearer/network setup success;
//   * provides one deterministic fake TCP socket;
//   * reports TCP connect success;
//   * accepts outbound bytes and logs a bounded prefix for protocol recovery;
//   * reports M_E_WOULDBLOCK for reads while no local response is queued; and
//   * accepts read/write callback registration.
//
// This is deliberately a protocol-discovery bridge, not a fabricated generic
// network stack.  Once the title emits its real cash-shop request, diagnostics
// give us the exact packet contract needed for a later local shop response.
const INOTIA1_AID: &str = "010100D3";
const INOTIA1_PID: &str = "PD005362";
const INOTIA1_FAKE_SOCKET_FD: i32 = 0;
const M_E_WOULDBLOCK: i32 = -19;


// Phase 8.34 — bounded persisted-character-name recovery.
//
// Phase 8.33 proved that the two corrupt scenario names are not resident in
// the 16/32-byte bucket allocations at cash-shop open.  Do not guess another
// fixed pointer.  Instead, before any synthetic catalog strings are injected,
// inspect only three bounded title-owned sources:
//   * the exact mapped Inotia 1 native image/data/BSS range;
//   * WIE allocations that are currently marked in-use; and
//   * structures reachable from the title's live r10/GOT static base.
//
// The native image on disk contains neither corrupt Korean label, so a unique
// exact pair appearing in the mapped native data/BSS is runtime state and can
// be repaired safely in place.  Heap/GOT matches are diagnostic only because
// a legitimately owned cash item can carry the same base label.  Fully
// rendered "name(class)" strings are handled separately by the exact RVCT
// string-helper diagnostic in wie_core_arm.
const INOTIA1_CORRUPT_NAME0_BASE: [u8; 12] = [
    0xc0, 0xda, 0xbf, 0xf8, 0x20, 0xb1, 0xb3, 0xc8, 0xaf, 0xb1, 0xc7, 0x00,
];
const INOTIA1_CORRECT_NAME0_BASE_PADDED: [u8; 12] = [
    0xc0, 0xcc, 0xb3, 0xeb, 0xc6, 0xbc, 0xbe, 0xc6, 0x00, 0x00, 0x00, 0x00,
];
const INOTIA1_CORRUPT_NAME1_BASE: [u8; 19] = [
    0xc3, 0xca, 0xba, 0xb8, 0xbf, 0xeb, 0x20, 0xbf, 0xeb, 0xbb, 0xe7, 0xc0,
    0xc7, 0x20, 0xc0, 0xce, 0xc0, 0xe5, 0x00,
];
const INOTIA1_CORRECT_NAME1_BASE_PADDED: [u8; 19] = [
    0xb1, 0xe2, 0xbb, 0xe7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const INOTIA1_NATIVE_BASE: WIPICWord = 0x0010_0000;
const INOTIA1_NATIVE_MAPPED_END: WIPICWord = 0x0018_b0c4;
const INOTIA1_LIST_HEAP_BASE: WIPICWord = 0x4000_0000;
const INOTIA1_LIST_HEAP_END: WIPICWord = 0x4800_0000;
const INOTIA1_GOT_SCAN_MAX_OFFSET: WIPICWord = 0x0800;
const INOTIA1_GOT_TARGET_SCAN_LEN: usize = 0x0400;

fn phase8_34_push_unique(hits: &mut Vec<WIPICWord>, address: WIPICWord) {
    if !hits.iter().any(|&existing| existing == address) {
        hits.push(address);
    }
}

fn phase8_34_find_matches_in_bytes(
    base: WIPICWord,
    data: &[u8],
    needle: &[u8],
    hits: &mut Vec<WIPICWord>,
) {
    if needle.is_empty() || needle.len() > data.len() {
        return;
    }
    for off in 0..=data.len() - needle.len() {
        if &data[off..off + needle.len()] == needle {
            phase8_34_push_unique(hits, base.wrapping_add(off as WIPICWord));
        }
    }
}

fn phase8_34_find_native_matches(
    context: &mut dyn WIPICContext,
    needle: &[u8],
) -> Vec<WIPICWord> {
    let len = INOTIA1_NATIVE_MAPPED_END.wrapping_sub(INOTIA1_NATIVE_BASE) as usize;
    let mut data = vec![0u8; len];
    if context.read_bytes(INOTIA1_NATIVE_BASE, &mut data).is_err() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    phase8_34_find_matches_in_bytes(INOTIA1_NATIVE_BASE, &data, needle, &mut hits);
    hits
}

// Phase 8.35 — early main-name repair used by the first few Inotia 1 repaint
// calls. This deliberately scans only the mapped native data/BSS range, where
// the original on-disk image contains no `자원 교환권` bytes. A unique runtime
// match is therefore safe to replace without touching item-database strings.
pub(crate) fn phase8_35_try_repair_main_name_native(
    context: &mut dyn WIPICContext,
) -> Result<bool> {
    let native0 = phase8_34_find_native_matches(context, &INOTIA1_CORRUPT_NAME0_BASE);
    if native0.len() != 1 {
        return Ok(false);
    }
    context.write_bytes(native0[0], &INOTIA1_CORRECT_NAME0_BASE_PADDED)?;
    tracing::info!(
        "[PHASE8_35_INOTIA1_MAIN_NAME_EARLY_REPAIR] ptr={:#010x} 자원 교환권 -> 이노티아; native runtime exact unique match",
        native0[0]
    );
    Ok(true)
}

fn phase8_34_find_allocated_bucket_matches(
    context: &mut dyn WIPICContext,
    header_address: WIPICWord,
    header_len: usize,
    slot_size: usize,
    slot_count: usize,
    needle: &[u8],
    hits: &mut Vec<WIPICWord>,
) {
    if needle.len() > slot_size {
        return;
    }
    let mut header = vec![0u8; header_len];
    if context.read_bytes(header_address, &mut header).is_err() {
        return;
    }
    let payload = header_address.wrapping_add(header_len as WIPICWord);
    for slot in 0..slot_count {
        let byte = header[slot / 8];
        let bit = 1u8 << (slot & 7);
        // BucketAllocator uses 0=allocated, 1=free.
        if byte & bit != 0 {
            continue;
        }
        let address = payload.wrapping_add((slot * slot_size) as WIPICWord);
        let mut data = vec![0u8; slot_size];
        if context.read_bytes(address, &mut data).is_err() {
            continue;
        }
        phase8_34_find_matches_in_bytes(address, &data, needle, hits);
    }
}

fn phase8_34_find_list_heap_matches(
    context: &mut dyn WIPICContext,
    needle: &[u8],
    hits: &mut Vec<WIPICWord>,
) {
    let mut cursor = INOTIA1_LIST_HEAP_BASE;
    let mut blocks = 0usize;
    while cursor < INOTIA1_LIST_HEAP_END && blocks < 0x20_000 {
        let Some(raw) = read_guest_u32(context, cursor) else {
            break;
        };
        let size = (raw & 0x7fff_ffff) as usize;
        let in_use = raw & 0x8000_0000 != 0;
        if size < 8 || size & 3 != 0 {
            break;
        }
        let Some(next) = cursor.checked_add(size as WIPICWord) else {
            break;
        };
        if next > INOTIA1_LIST_HEAP_END || next <= cursor {
            break;
        }

        // Layout is 4-byte header + payload/padding + 4-byte canary.  Avoid a
        // pathological allocation from turning this diagnostic into a large
        // copy; character/save structures are far smaller than 2 MiB.
        if in_use {
            let payload_len = size.saturating_sub(8);
            if payload_len >= needle.len() && payload_len <= 0x20_0000 {
                let address = cursor.wrapping_add(4);
                let mut data = vec![0u8; payload_len];
                if context.read_bytes(address, &mut data).is_ok() {
                    phase8_34_find_matches_in_bytes(address, &data, needle, hits);
                }
            }
        }
        cursor = next;
        blocks += 1;
    }
}

fn phase8_34_find_allocated_heap_matches(
    context: &mut dyn WIPICContext,
    needle: &[u8],
) -> Vec<WIPICWord> {
    let mut hits = Vec::new();
    // Bucket allocator occupies 0x48000000.. and uses the layout defined in
    // wie_core_arm::allocator::bucket. Scan only in-use slots large enough to
    // contain the complete NUL-terminated string.
    let buckets: [(WIPICWord, usize, usize, usize); 6] = [
        (0x4883_0000, 0x1_0000usize, 16usize, 0x8_0000usize),
        (0x4904_0000, 0x8000usize, 32usize, 0x4_0000usize),
        (0x4984_8000, 0x8000usize, 64usize, 0x4_0000usize),
        (0x4a85_0000, 0x4000usize, 128usize, 0x2_0000usize),
        (0x4b85_4000, 0x4000usize, 256usize, 0x2_0000usize),
        (0x4d85_8000, 0x2000usize, 512usize, 0x1_0000usize),
    ];
    for (header, header_len, slot_size, slot_count) in buckets {
        phase8_34_find_allocated_bucket_matches(
            context,
            header,
            header_len,
            slot_size,
            slot_count,
            needle,
            &mut hits,
        );
    }
    phase8_34_find_list_heap_matches(context, needle, &mut hits);
    hits
}

fn phase8_34_plausible_guest_pointer(value: WIPICWord) -> bool {
    (INOTIA1_NATIVE_BASE..INOTIA1_NATIVE_MAPPED_END).contains(&value)
        || (0x4000_0000..0x5000_0000).contains(&value)
}

fn phase8_34_scan_pointer_target(
    context: &mut dyn WIPICContext,
    pointer: WIPICWord,
    needle: &[u8],
    hits: &mut Vec<WIPICWord>,
) {
    if !phase8_34_plausible_guest_pointer(pointer) {
        return;
    }
    let mut data = vec![0u8; INOTIA1_GOT_TARGET_SCAN_LEN];
    if context.read_bytes(pointer, &mut data).is_ok() {
        phase8_34_find_matches_in_bytes(pointer, &data, needle, hits);
    }
}

fn phase8_34_find_got_reachable_matches(
    context: &mut dyn WIPICContext,
    r10: WIPICWord,
    needle: &[u8],
) -> Vec<WIPICWord> {
    let mut hits = Vec::new();
    let mut scanned = Vec::new();
    for offset in (0u32..=INOTIA1_GOT_SCAN_MAX_OFFSET).step_by(4) {
        let Some(pointer) = read_guest_u32(context, r10.wrapping_add(offset)) else {
            continue;
        };
        if phase8_34_plausible_guest_pointer(pointer)
            && !scanned.iter().any(|&existing| existing == pointer)
        {
            scanned.push(pointer);
            phase8_34_scan_pointer_target(context, pointer, needle, &mut hits);

            // Many GOT entries point to a global pointer rather than directly
            // to the object. Follow one additional level, never recursively.
            if let Some(nested) = read_guest_u32(context, pointer) {
                if phase8_34_plausible_guest_pointer(nested)
                    && !scanned.iter().any(|&existing| existing == nested)
                {
                    scanned.push(nested);
                    phase8_34_scan_pointer_target(context, nested, needle, &mut hits);
                }
            }
        }
    }
    hits
}

fn phase8_34_probe_and_repair_inotia1_names(context: &mut dyn WIPICContext) -> Result<()> {
    let r10 = context.debug_cpu_context().map(|cpu| cpu[10]).unwrap_or(0);

    let native0 = phase8_34_find_native_matches(context, &INOTIA1_CORRUPT_NAME0_BASE);
    let native1 = phase8_34_find_native_matches(context, &INOTIA1_CORRUPT_NAME1_BASE);
    let heap0 = phase8_34_find_allocated_heap_matches(context, &INOTIA1_CORRUPT_NAME0_BASE);
    let heap1 = phase8_34_find_allocated_heap_matches(context, &INOTIA1_CORRUPT_NAME1_BASE);
    let got0 = if r10 != 0 {
        phase8_34_find_got_reachable_matches(context, r10, &INOTIA1_CORRUPT_NAME0_BASE)
    } else {
        Vec::new()
    };
    let got1 = if r10 != 0 {
        phase8_34_find_got_reachable_matches(context, r10, &INOTIA1_CORRUPT_NAME1_BASE)
    } else {
        Vec::new()
    };

    tracing::info!(
        "[PHASE8_35_INOTIA1_NAME_BOUNDED_PROBE] r10={r10:#010x} native0={native0:?} native1={native1:?} heap0={heap0:?} heap1={heap1:?} got0={got0:?} got1={got1:?}"
    );

    // Phase 8.35 focuses on the persisted main-character name only. The
    // unmodified native image contains no "자원 교환권" label, so one exact
    // runtime copy in mapped data/BSS is sufficient evidence; do not require
    // the secondary hero slot to be corrupt as well. This lets a newly recruited
    // replacement hero remain entirely under the game's control.
    if native0.len() == 1 {
        context.write_bytes(native0[0], &INOTIA1_CORRECT_NAME0_BASE_PADDED)?;
        tracing::info!(
            "[PHASE8_35_INOTIA1_MAIN_NAME_REPAIR] mode=native-runtime name0_ptr={:#010x} 자원 교환권 -> 이노티아; secondary hero untouched",
            native0[0]
        );
    } else {
        tracing::info!(
            "[PHASE8_35_INOTIA1_MAIN_NAME_REPAIR] no write: native main-name candidate count={}; heap/GOT candidates remain diagnostic-only",
            native0.len()
        );
    }
    Ok(())
}

// Phase 8.22 — corrected server-first framing for the original KTF cash shop.
//
// Static Thumb disassembly of the packet dispatcher at guest 0x00117194 shows
// that *every* received frame first consumes one common one-byte result/state
// field and stores it through the GOT slot at r10+0x470 before dispatching on
// the command byte. Command 0 only builds the authentic command-1 request when
// that field is 1. Earlier phases sent a three-byte command-0 frame with no
// payload, leaving the field at 0 and then forced the request-builder branch.
// Feed the byte the original state machine expects instead: length=4,
// command=0, common result/state=1. No live carrier server is contacted.
const INOTIA1_CASH_SERVER_HELLO: [u8; 4] = [0x00, 0x04, 0x00, 0x01];

// First authentic client request recovered from the title:
//   00 14 01 0b "01012349876" 01 00 00 00 64
//
// The same common one-byte field precedes command 1's command-specific parser.
// After that byte, the native command-1 parser consumes exactly 24 more bytes:
//   u32, u32, u8, u8, u32, u32, string8, string8, u32 status
// with empty strings in this minimal offline response. Therefore the correct
// frame is 28 bytes, not the 27-byte Phase 8.18 experiment. The leading result
// field is 1 so the original command-1 handler enters its real parser instead
// of its early error-2009/state-5 branch at guest 0x00117258.
const INOTIA1_CASH_CMD1_SUCCESS: [u8; 28] = [
    0x00, 0x1c, 0x01, // length=28, command=1
    0x01, // common result/state field consumed before command dispatch
    0x00, 0x00, 0x00, 0x00, // field 1
    0x00, 0x00, 0x00, 0x00, // field 2
    0x00, // field 3
    0x00, // field 4
    0x00, 0x00, 0x00, 0x00, // field 5
    0x00, 0x00, 0x00, 0x00, // field 6
    0x00, // string 1 length
    0x00, // string 2 length
    0x00, 0x00, 0x00, 0x00, // status=success
];

// Phase 8.23 — first response stage after the authentic command-5 request.
//
// Phase 8.22 is the first build that gets through the original command-0 and
// command-1 handshake far enough for the title to emit:
//   00 0c 05 00 00 00 00 00 00 00 01 01
// The native receive switch does not have a command-5 response parser. Static
// analysis resolves the next server-to-client stage as command 2. Its handler
// first requires common result/state=1, then reads a big-endian u16 byte-count
// and copies exactly that many bytes from the frame. Keep this first probe
// deliberately empty (count=0) so the original state machine can advance
// without inventing catalog records or touching char/save inventory data.
// A later command-3/4 continuation will only be synthesized after the next
// field log reveals which stage the client requests/awaits after this frame.
const INOTIA1_CASH_CMD5_STAGE2_EMPTY: [u8; 6] = [
    0x00, 0x06, 0x02, // length=6, server command=2
    0x01, // common result/state = success
    0x00, 0x00, // command-2 data length = 0
];

// Phase 8.24 — complete the zero-byte transfer instead of leaving the native
// cash-shop state machine halfway through command 2. Static analysis of the
// command-4 finalizer (guest 0x00117744) shows that, after the common success
// byte, it consumes one type byte, a one-byte string length (zero is valid),
// then one final flag byte. With no trailing bytes the parser cleanly reaches
// its original transfer-finalization path. This still represents an *empty*
// catalog; it does not create items or modify save/inventory data.
const INOTIA1_CASH_CMD5_STAGE4_FINALIZE_EMPTY: [u8; 7] = [
    0x00, 0x07, 0x04, // length=7, server command=4
    0x01, // common result/state = success
    0x00, // transfer/catalog type
    0x00, // zero-length string
    0x00, // final flag
];

// Phase 8.52 — stabilized offline catalog with bulk enhancement scrolls + bulk materials.
//
// Keep the Phase 8.50 utility/catalog behavior, add the two native enhancement
// scroll items resolved from the title's original work.bar, and retain x10 for
// 힘의 조각 / 마법의 가지.  The two enhancement scrolls and the two material items each use quantity 10; all other records keep quantity 1 and price 0.
// Eleven records remain below the native 12-record capacity, so both
// 부활주문서 and 축복받은 부활주문서 can remain available.
const INOTIA1_CASH_CMD30_CATALOG_SINGLE_PAGE_FREE: [u8; 209] = [
    0x00, 0xd1, 0x1e, 0x01, 0x00, 0x01, 0x0b, 0x06, 0xbd, 0xba, 0xc5, 0xb3,
    0xba, 0xcf, 0x01, 0x00, 0x00, 0x00, 0x00, 0x0a, 0xba, 0xce, 0xc8, 0xb0,
    0xc1, 0xd6, 0xb9, 0xae, 0xbc, 0xad, 0x01, 0x00, 0x00, 0x00, 0x00, 0x13,
    0xc3, 0xe0, 0xba, 0xb9, 0xb9, 0xde, 0xc0, 0xba, 0x20, 0xba, 0xce, 0xc8,
    0xb0, 0xc1, 0xd6, 0xb9, 0xae, 0xbc, 0xad, 0x01, 0x00, 0x00, 0x00, 0x00,
    0x09, 0xbb, 0xf3, 0xc0, 0xda, 0x20, 0xbf, 0xad, 0xbc, 0xe8, 0x01, 0x00,
    0x00, 0x00, 0x00, 0x14, 0xc3, 0xe0, 0xba, 0xb9, 0xb9, 0xde, 0xc0, 0xba,
    0x20, 0xbf, 0xeb, 0xbb, 0xe7, 0xc0, 0xc7, 0x20, 0xc0, 0xce, 0xc0, 0xe5,
    0x01, 0x00, 0x00, 0x00, 0x00, 0x09, 0x31, 0x36, 0xc4, 0xad, 0x20, 0xb0,
    0xa1, 0xb9, 0xe6, 0x01, 0x00, 0x00, 0x00, 0x00, 0x0b, 0xbd, 0xba, 0xc5,
    0xb3, 0x20, 0xc3, 0xca, 0xb1, 0xe2, 0xc8, 0xad, 0x01, 0x00, 0x00, 0x00,
    0x00, 0x0f, 0xb9, 0xab, 0xb1, 0xe2, 0xb0, 0xad, 0xc8, 0xad, 0x20, 0xc1,
    0xd6, 0xb9, 0xae, 0xbc, 0xad, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x11, 0xb9,
    0xe6, 0xbe, 0xee, 0xb1, 0xb8, 0xb0, 0xad, 0xc8, 0xad, 0x20, 0xc1, 0xd6,
    0xb9, 0xae, 0xbc, 0xad, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x09, 0xc8, 0xfb,
    0xc0, 0xc7, 0x20, 0xc1, 0xb6, 0xb0, 0xa2, 0x0a, 0x00, 0x00, 0x00, 0x00,
    0x0b, 0xb8, 0xb6, 0xb9, 0xfd, 0xc0, 0xc7, 0x20, 0xb0, 0xa1, 0xc1, 0xf6,
    0x0a, 0x00, 0x00, 0x00, 0x00,
];

// Phase 8.38 — party-wipe emergency prayer catalog.
//
// Static recovery of the original title proves item id 0x219 (537) is
// "부활의 기도문".  The party-wipe input state machine enters state 14 only
// when the user selected the prayer-revival branch but does not own the item;
// that state then launches the carrier cash shop. Keep the current normal
// single-page catalog separate, but while that exact death state is active,
// expose only the missing prayer as a one-record emergency catalog. The
// packet uses the same [0,1] compatibility metadata that field testing
// validated for normal shop entry, and remains far below the 12-record native
// table ceiling.
const INOTIA1_CASH_CMD30_EMERGENCY_PRAYER_FREE: [u8; 26] = [
    0x00, 0x1a, 0x1e, 0x01, 0x00, 0x01, 0x01,
    0x0d, 0xba, 0xce, 0xc8, 0xb0, 0xc0, 0xc7, 0x20, 0xb1, 0xe2, 0xb5, 0xb5,
    0xb9, 0xae, 0x01, 0x00, 0x00, 0x00, 0x00,
];

// Phase 8.26 — command-31 is the title's authentic BUY request. The exact
// receive handler at guest 0x001181b8 accepts common result/state=1 and, on its
// purchase-completion path, consumes one four-byte value before executing the
// original inventory/UI completion logic. Return zero as the post-purchase
// balance/value and allow the game itself to perform any inventory mutation.
const INOTIA1_CASH_CMD31_PURCHASE_SUCCESS: [u8; 8] = [
    0x00, 0x08, 0x1f, // length=8, server command=31
    0x01, // common result/state = success
    0x00, 0x00, 0x00, 0x00, // post-purchase value/balance = 0
];

// Phase 8.34 — 자원 교환권 is a server/account resource exchange, not a
// normal single-player inventory grant. Field testing with the Phase 8.30
// synthetic success showed the client displaying "10 resources" and saving,
// but no inventory/resource became usable offline. Do not acknowledge success
// and potentially consume a ticket for a balance that cannot exist locally.
// Return the authentic command with common result/state=0 so the original
// client follows its rejection path without timing out.
const INOTIA1_CMD89_RESOURCE_EXCHANGE_OFFLINE_REJECT: [u8; 4] = [
    0x00, 0x04, 0x59, 0x00, // length=4, command=89, common result/state=failure
];

// Phase 8.34 — command 123 is not one-way. Static recovery of the native
// receive jump table proves server command 123 routes to the common cleanup
// handler at guest 0x001171fa. The client still consumes the common one-byte
// result/state field before dispatch, so this is the minimal native cleanup
// success response after the observed outbound 00 04 7b 00 exit packet.
const INOTIA1_CMD123_CLEANUP_SUCCESS: [u8; 4] = [
    0x00, 0x04, 0x7b, 0x01,
];

const CASH_RX_HELLO: u8 = 0;
const CASH_RX_WAIT: u8 = 1;
const CASH_RX_CMD1: u8 = 2;
const CASH_RX_CMD5_STAGE2: u8 = 3;
const CASH_RX_CMD5_STAGE4: u8 = 4;
const CASH_RX_CMD30_REENTRY: u8 = 5;
const CASH_RX_CMD31_PURCHASE: u8 = 6;
const CASH_RX_CMD89_RESOURCE_EXCHANGE: u8 = 7;
const CASH_RX_CMD123_CLEANUP: u8 = 8;

#[derive(Copy, Clone)]
struct Inotia1CashRxState {
    phase: u8,
    offset: usize,
}

static INOTIA1_CASH_RX_STATE: Mutex<Inotia1CashRxState> = Mutex::new(Inotia1CashRxState {
    phase: CASH_RX_HELLO,
    offset: 0,
});

static INOTIA1_CASH_CATALOG_PAGE: Mutex<u8> = Mutex::new(0);

// Phase 8.40: this is a latched party-wipe purchase-flow flag. Phase 8.38
// correctly detected native death state 14 at the first command-5 boundary,
// but the title immediately reconnects after moving its outer state to 6.
// Preserve the latch across those reconnects and socket closes while the
// purchase is still pending. Once an emergency command-31 SUCCESS frame is
// fully delivered, the purchase is no longer a cancellation case: clear this
// latch and queue an immediate command-123 cleanup so the title's own
// state-13/state-1 post-purchase path owns the return instead of our CLEAR
// recovery overwriting it.
static INOTIA1_EMERGENCY_PRAYER_CASH: AtomicBool = AtomicBool::new(false);

pub fn phase8_40_inotia1_emergency_prayer_cash_active() -> bool {
    INOTIA1_EMERGENCY_PRAYER_CASH.load(Ordering::Relaxed)
}

pub fn phase8_40_clear_inotia1_emergency_prayer_cash_latch() {
    INOTIA1_EMERGENCY_PRAYER_CASH.store(false, Ordering::Relaxed);
}

fn inotia1_cash_catalog_frame(_page: u8) -> &'static [u8] {
    if INOTIA1_EMERGENCY_PRAYER_CASH.load(Ordering::Relaxed) {
        &INOTIA1_CASH_CMD30_EMERGENCY_PRAYER_FREE
    } else {
        &INOTIA1_CASH_CMD30_CATALOG_SINGLE_PAGE_FREE
    }
}

// Phase 8.25 — persist the legacy read-callback registration.
//
// Phase 8.24 proved that the client can leave the synchronous command-2/4
// transfer, register MC_netSetReadCB, and only later emit command 30 from its
// main timer thread. The offline bridge queued the command-30 response, but
// set_read_cb previously discarded the callback pointer, so nothing woke the
// guest to call slot-32 RECV. The title then hit its own 30-second timeout
// (cash error 2014). Keep the latest one-shot waiter and fire it when local
// response data becomes available.
#[derive(Copy, Clone)]
struct Inotia1CashReadCallback {
    active: bool,
    fd: i32,
    cb: WIPICWord,
    param: WIPICWord,
}

static INOTIA1_CASH_READ_CALLBACK: Mutex<Inotia1CashReadCallback> =
    Mutex::new(Inotia1CashReadCallback {
        active: false,
        fd: INOTIA1_FAKE_SOCKET_FD,
        cb: 0,
        param: 0,
    });

fn reset_inotia1_cash_read_callback() {
    *INOTIA1_CASH_READ_CALLBACK.lock() = Inotia1CashReadCallback {
        active: false,
        fd: INOTIA1_FAKE_SOCKET_FD,
        cb: 0,
        param: 0,
    };
}

fn inotia1_cash_response_pending() -> bool {
    let state = *INOTIA1_CASH_RX_STATE.lock();
    let frame_len = match state.phase {
        CASH_RX_HELLO => INOTIA1_CASH_SERVER_HELLO.len(),
        CASH_RX_CMD1 => INOTIA1_CASH_CMD1_SUCCESS.len(),
        CASH_RX_CMD5_STAGE2 => INOTIA1_CASH_CMD5_STAGE2_EMPTY.len(),
        CASH_RX_CMD5_STAGE4 => INOTIA1_CASH_CMD5_STAGE4_FINALIZE_EMPTY.len(),
        CASH_RX_CMD30_REENTRY => inotia1_cash_catalog_frame(*INOTIA1_CASH_CATALOG_PAGE.lock()).len(),
        CASH_RX_CMD31_PURCHASE => INOTIA1_CASH_CMD31_PURCHASE_SUCCESS.len(),
        CASH_RX_CMD89_RESOURCE_EXCHANGE => INOTIA1_CMD89_RESOURCE_EXCHANGE_OFFLINE_REJECT.len(),
        CASH_RX_CMD123_CLEANUP => INOTIA1_CMD123_CLEANUP_SUCCESS.len(),
        _ => 0,
    };
    frame_len != 0 && state.offset < frame_len
}

fn wake_inotia1_cash_reader(
    context: &mut dyn WIPICContext,
    reason: &'static str,
) -> Result<()> {
    if !inotia1_cash_response_pending() {
        return Ok(());
    }

    // The observed callback is 0x0010c15d. Keep the same exact-title native
    // range guard used for the legacy socket-connect callback before jumping
    // through any guest-provided function pointer.
    const INOTIA1_NATIVE_START: WIPICWord = 0x0010_0001;
    const INOTIA1_NATIVE_END: WIPICWord = 0x0018_b0c5;

    let waiter = {
        let mut slot = INOTIA1_CASH_READ_CALLBACK.lock();
        if !slot.active {
            return Ok(());
        }
        let waiter = *slot;
        slot.active = false; // one wake per registration; guest may re-arm.
        waiter
    };

    let callback_is_safe = (waiter.cb & 1) == 1
        && waiter.cb >= INOTIA1_NATIVE_START
        && waiter.cb < INOTIA1_NATIVE_END;
    if !callback_is_safe {
        tracing::warn!(
            "[PHASE8_25_INOTIA1_CASH_READ_WAKE] unsafe callback suppressed cb={:#010x} reason={reason}",
            waiter.cb
        );
        return Ok(());
    }

    struct CashReadReadyCallback {
        fd: i32,
        cb: WIPICWord,
        param: WIPICWord,
        reason: &'static str,
    }

    #[async_trait::async_trait]
    impl MethodBody<WieError> for CashReadReadyCallback {
        #[tracing::instrument(name = "timer", skip_all)]
        async fn call(
            &self,
            context: &mut dyn WIPICContext,
            _: Box<[WIPICWord]>,
        ) -> Result<WIPICResult> {
            // Preserve the asynchronous ordering expected by the carrier API;
            // callback ABI is (fd, status, param), with status=0 meaning data
            // is ready. The guest callback then invokes legacy slot-32 RECV.
            context.system().sleep(1).await;
            tracing::info!(
                "[PHASE8_25_INOTIA1_CASH_READ_WAKE] callback={:#010x} fd={} status=0 param={:#010x} reason={}",
                self.cb,
                self.fd,
                self.param,
                self.reason
            );
            context
                .call_function(self.cb, &[self.fd as WIPICWord, 0, self.param])
                .await?;
            Ok(WIPICResult { results: Vec::new() })
        }
    }

    context.spawn(Box::new(CashReadReadyCallback {
        fd: waiter.fd,
        cb: waiter.cb,
        param: waiter.param,
        reason,
    }))?;
    Ok(())
}

fn reset_inotia1_cash_rx() {
    *INOTIA1_CASH_RX_STATE.lock() = Inotia1CashRxState {
        phase: CASH_RX_HELLO,
        offset: 0,
    };
}

fn queue_inotia1_cash_cmd1_success() {
    *INOTIA1_CASH_RX_STATE.lock() = Inotia1CashRxState {
        phase: CASH_RX_CMD1,
        offset: 0,
    };
}

fn queue_inotia1_cash_cmd5_stage2() {
    *INOTIA1_CASH_RX_STATE.lock() = Inotia1CashRxState {
        phase: CASH_RX_CMD5_STAGE2,
        offset: 0,
    };
}

fn reset_inotia1_cash_session_wait() {
    *INOTIA1_CASH_RX_STATE.lock() = Inotia1CashRxState {
        phase: CASH_RX_WAIT,
        offset: 0,
    };
}

fn queue_inotia1_cash_cmd30_page(page: u8) {
    *INOTIA1_CASH_CATALOG_PAGE.lock() = 0;
    let _ = page;
    *INOTIA1_CASH_RX_STATE.lock() = Inotia1CashRxState {
        phase: CASH_RX_CMD30_REENTRY,
        offset: 0,
    };
}

fn queue_inotia1_cash_cmd31_purchase_success() {
    *INOTIA1_CASH_RX_STATE.lock() = Inotia1CashRxState {
        phase: CASH_RX_CMD31_PURCHASE,
        offset: 0,
    };
}

fn queue_inotia1_cmd89_resource_exchange_reject() {
    *INOTIA1_CASH_RX_STATE.lock() = Inotia1CashRxState {
        phase: CASH_RX_CMD89_RESOURCE_EXCHANGE,
        offset: 0,
    };
}

fn queue_inotia1_cmd123_cleanup_success() {
    *INOTIA1_CASH_RX_STATE.lock() = Inotia1CashRxState {
        phase: CASH_RX_CMD123_CLEANUP,
        offset: 0,
    };
}

fn is_inotia1_offline_network(context: &mut dyn WIPICContext) -> bool {
    let system = context.system();
    system.aid() == INOTIA1_AID && system.pid() == INOTIA1_PID
}

fn read_guest_u32(context: &mut dyn WIPICContext, address: WIPICWord) -> Option<u32> {
    let mut bytes = [0u8; 4];
    context.read_bytes(address, &mut bytes).ok()?;
    Some(u32::from_le_bytes(bytes))
}

fn read_inotia1_got_u32(
    context: &mut dyn WIPICContext,
    r10: WIPICWord,
    got_offset: WIPICWord,
) -> Option<u32> {
    let ptr = read_guest_u32(context, r10.wrapping_add(got_offset))?;
    read_guest_u32(context, ptr)
}

fn write_guest_u32(context: &mut dyn WIPICContext, address: WIPICWord, value: u32) -> bool {
    context.write_bytes(address, &value.to_le_bytes()).is_ok()
}

fn write_inotia1_got_u32(
    context: &mut dyn WIPICContext,
    r10: WIPICWord,
    got_offset: WIPICWord,
    value: u32,
) -> bool {
    let Some(ptr) = read_guest_u32(context, r10.wrapping_add(got_offset)) else {
        return false;
    };
    write_guest_u32(context, ptr, value)
}

// Phase 8.38 — exact party-wipe state globals recovered from native code:
//   r10+0x25c -> outer death/revival UI state
//   r10+0x30c -> death animation/substate
//   r10+0x5f8 -> two-option selection in the death/prayer prompt
// State 14 is entered by guest 0x0014a706 only when item 0x219 cannot be
// found and is the native "buy 부활의 기도문" prompt.  Importantly, the
// original CLEAR path at 0x0014a606 performs exactly state=11, selection=0.
fn inotia1_party_wipe_state(
    context: &mut dyn WIPICContext,
) -> Option<(WIPICWord, u32, u32, u32)> {
    let cpu = context.debug_cpu_context()?;
    let r10 = cpu[10];
    let state = read_inotia1_got_u32(context, r10, 0x25c)?;
    let substate = read_inotia1_got_u32(context, r10, 0x30c).unwrap_or(u32::MAX);
    let selection = read_inotia1_got_u32(context, r10, 0x5f8).unwrap_or(u32::MAX);
    Some((r10, state, substate, selection))
}

fn phase8_40_detect_emergency_prayer_cash(context: &mut dyn WIPICContext, label: &str) -> bool {
    let Some((r10, state, substate, selection)) = inotia1_party_wipe_state(context) else {
        tracing::info!(
            "[PHASE8_40_INOTIA1_WIPE_CASH_STATE] label={label} unavailable -> normal catalog"
        );
        return false;
    };
    let emergency = state == 14;
    tracing::info!(
        "[PHASE8_40_INOTIA1_WIPE_CASH_STATE] label={label} r10={r10:#010x} state={state} substate={substate} selection={selection} emergency={emergency}"
    );
    emergency
}

fn phase8_40_restore_death_prompt_after_cash_cancel(context: &mut dyn WIPICContext) {
    let Some((r10, state, substate, selection)) = inotia1_party_wipe_state(context) else {
        tracing::warn!(
            "[PHASE8_40_INOTIA1_WIPE_CASH_CANCEL_RECOVERY] state unavailable; no guest memory changed"
        );
        return;
    };

    // The latch proves this cash flow originated from the title's native
    // state-14 missing-prayer branch.  During the nested cash reconnect the
    // title itself changes the outer state to 6, so requiring `state == 14`
    // (Phase 8.38) misses the actual CLEAR/back moment.  Restore the same
    // destination as the original state-14 CLEAR handler even when the nested
    // overlay currently exposes state 6.  Do not revive anyone, consume an
    // item, apply a penalty, or touch save bytes.
    let state_ok = write_inotia1_got_u32(context, r10, 0x25c, 11);
    let selection_ok = write_inotia1_got_u32(context, r10, 0x5f8, 0);
    tracing::info!(
        "[PHASE8_40_INOTIA1_WIPE_CASH_CANCEL_RECOVERY] latched emergency flow restored state {state}->11 selection {selection}->0 substate={substate} state_write={state_ok} selection_write={selection_ok}"
    );
}

// Phase 8.34 — trace the corrected command-30 page globals. Static UI/native
// analysis proves GOT 0x510 is page_count, GOT 0x4a4 is zero-based current
// page, and GOT 0x4d8 is record_count.
fn trace_phase8_34_inotia1_page_globals(
    context: &mut dyn WIPICContext,
    requested_page: u8,
    label: &str,
) {
    if let Some(cpu) = context.debug_cpu_context() {
        let r10 = cpu[10];
        let field_510 = read_inotia1_got_u32(context, r10, 0x510);
        let field_4a4 = read_inotia1_got_u32(context, r10, 0x4a4);
        let record_count = read_inotia1_got_u32(context, r10, 0x4d8);
        tracing::info!(
            "[PHASE8_34_INOTIA1_PAGE_GLOBALS] label={label} requested={requested_page} r10={r10:#010x} got510={field_510:?} got4a4={field_4a4:?} record_count={record_count:?} pc={:#010x} lr={:#010x}",
            cpu[15], cpu[14]
        );
    }
}

// Phase 8.22 — diagnostic for the common one-byte response state/result field.
// The native packet dispatcher stores this field through the pointer held at
// r10+0x470 before entering each command-specific handler. Reading it here lets
// the next field test prove that the corrected command-0/command-1 frames are
// actually driving the title's original state machine with value 1.
fn trace_inotia1_cash_protocol_state(context: &mut dyn WIPICContext, label: &str) {
    const RESPONSE_STATE_GOT_OFFSET: WIPICWord = 0x470;
    if let Some(cpu) = context.debug_cpu_context() {
        let r10 = cpu[10];
        let value = read_inotia1_got_u32(context, r10, RESPONSE_STATE_GOT_OFFSET);
        tracing::info!(
            "[PHASE8_22_INOTIA1_CASH_RESPONSE_STATE] label={label} r10={r10:#010x} value={value:?}"
        );
    }
}

fn trace_inotia1_cash_reject(context: &mut dyn WIPICContext, api: &str, fd: Option<i32>) {
    let state = *INOTIA1_CASH_RX_STATE.lock();
    if let Some(cpu) = context.debug_cpu_context() {
        let lr = cpu[14];
        let pc = cpu[15];
        let r10 = cpu[10];
        let mut code = [0u8; 16];
        let code_base = (lr & !1).saturating_sub(8);
        let code_ok = context.read_bytes(code_base, &mut code).is_ok();

        // Phase 8.20 — the Inotia 1 network module keeps its current protocol
        // status/error in the GOT slot at r10+0x46c. The generic error handler
        // writes its r0 error code to this same global immediately before the
        // socket-close sequence. Dereference it here so a later rejection can
        // be attributed to the exact native error instead of only the common
        // cleanup call site. 0x2ec is the module state (the error path writes 5).
        let cash_code = read_inotia1_got_u32(context, r10, 0x46c);
        let cash_state = read_inotia1_got_u32(context, r10, 0x2ec);
        tracing::info!(
            "[PHASE8_20_INOTIA1_CASH_STATE] api={api} cash_code={cash_code:?} cash_state={cash_state:?}"
        );
        tracing::info!(
            "[PHASE8_19_INOTIA1_CASH_REJECT] api={api} fd={fd:?} phase={} offset={} pc={pc:#010x} lr={lr:#010x} r10={r10:#010x} r0={:#010x} r1={:#010x} r2={:#010x} r3={:#010x} code_base={code_base:#010x} code_ok={code_ok} code={code:02x?}",
            state.phase,
            state.offset,
            cpu[0],
            cpu[1],
            cpu[2],
            cpu[3],
        );
    } else {
        tracing::info!(
            "[PHASE8_19_INOTIA1_CASH_REJECT] api={api} fd={fd:?} phase={} offset={} cpu_context=unavailable",
            state.phase,
            state.offset
        );
    }
}

pub async fn connect(context: &mut dyn WIPICContext, cb: WIPICWord, param: WIPICWord) -> Result<i32> {
    let inotia1 = is_inotia1_offline_network(context);

    if inotia1 {
        reset_inotia1_cash_rx();
        reset_inotia1_cash_read_callback();
        tracing::info!(
            "[PHASE8_12_INOTIA1_NET] MC_netConnect offline bridge cb={cb:#010x} param={param:#010x} -> callback success"
        );
    } else {
        tracing::warn!("stub MC_netConnect({cb:#x}, {param:#x})");
    }

    struct ConnectCallback {
        cb: WIPICWord,
        param: WIPICWord,
        success: bool,
    }

    #[async_trait::async_trait]
    impl MethodBody<WieError> for ConnectCallback {
        #[tracing::instrument(name = "timer", skip_all)]
        async fn call(&self, context: &mut dyn WIPICContext, _: Box<[WIPICWord]>) -> Result<WIPICResult> {
            context.system().sleep(1).await; // preserve asynchronous WIPI callback ordering

            let error = if self.success { 0 } else { u32::MAX }; // 0 or M_E_ERROR
            context.call_function(self.cb, &[error, self.param]).await?;

            Ok(WIPICResult { results: Vec::new() })
        }
    }

    context.spawn(Box::new(ConnectCallback {
        cb,
        param,
        success: inotia1,
    }))?;

    Ok(0)
}

pub async fn close(context: &mut dyn WIPICContext) -> Result<()> {
    if is_inotia1_offline_network(context) {
        trace_inotia1_cash_reject(context, "MC_netClose", None);
        reset_inotia1_cash_rx();
        reset_inotia1_cash_read_callback();
        tracing::info!("[INOTIA1_CASH_NET] MC_netClose offline bridge");
    } else {
        tracing::warn!("stub MC_netClose()");
    }

    Ok(())
}

pub async fn socket(context: &mut dyn WIPICContext, domain: i32, socket_type: i32) -> Result<i32> {
    if is_inotia1_offline_network(context) {
        tracing::info!(
            "[INOTIA1_CASH_NET] MC_netSocket domain={domain} type={socket_type} -> fd={INOTIA1_FAKE_SOCKET_FD}"
        );
        return Ok(INOTIA1_FAKE_SOCKET_FD);
    }

    Err(WieError::Unimplemented("2: MC_netSocket".into()))
}

pub async fn socket_connect(
    context: &mut dyn WIPICContext,
    fd: i32,
    addr: WIPICWord,
    port: WIPICWord,
    cb: WIPICWord,
    param: WIPICWord,
) -> Result<i32> {
    if !is_inotia1_offline_network(context) {
        return Err(WieError::Unimplemented("3: MC_netSocketConnect".into()));
    }

    tracing::info!(
        "[INOTIA1_CASH_NET] MC_netSocketConnect fd={fd} addr={addr:#010x} port_raw={port:#06x} cb={cb:#010x} param={param:#010x} -> callback success (offline; no host connection)"
    );

    struct SocketConnectCallback {
        fd: i32,
        cb: WIPICWord,
        param: WIPICWord,
    }

    #[async_trait::async_trait]
    impl MethodBody<WieError> for SocketConnectCallback {
        #[tracing::instrument(name = "timer", skip_all)]
        async fn call(&self, context: &mut dyn WIPICContext, _: Box<[WIPICWord]>) -> Result<WIPICResult> {
            context.system().sleep(1).await;
            context
                .call_function(self.cb, &[self.fd as WIPICWord, 0, self.param])
                .await?;
            Ok(WIPICResult { results: Vec::new() })
        }
    }

    context.spawn(Box::new(SocketConnectCallback { fd, cb, param }))?;
    Ok(0)
}

// Phase 8.13 — KTF Inotia 1 uses a carrier-extension network entry at
// interface offset 0x78 (slot 30), after MC_netSocket and MC_utilInetAddrInt.  WIE's
// generic table previously ended at slot 29, so the guest dereferenced one
// word past the allocated method table and crashed before it could send the
// cash-shop request.
//
// Static analysis of PD005362 shows the call shape as six arguments:
//   (fd, host, port, flags, callback, callback_param)
// and accepts either immediate 0 or M_E_WOULDBLOCK as a non-failure return.
// The callback used by this title treats its second argument as the status.
// Keep this extension exact-title-only, report synchronous success, and queue
// a conservative asynchronous success callback when the callback is a valid
// Thumb pointer inside this title's native image.
pub async fn socket_connect_ktf_legacy(
    context: &mut dyn WIPICContext,
    fd: i32,
    addr: WIPICWord,
    port: WIPICWord,
    flags: WIPICWord,
    cb: WIPICWord,
    param: WIPICWord,
) -> Result<i32> {
    if !is_inotia1_offline_network(context) {
        return Err(WieError::Unimplemented(
            "30: KTF legacy MC_netSocketConnectAsync".into(),
        ));
    }

    tracing::info!(
        "[PHASE8_13_INOTIA1_NET30] fd={fd} addr={addr:#010x} port_raw={port:#06x} flags={flags:#010x} cb={cb:#010x} param={param:#010x} -> 0"
    );

    // Inotia 1's client.bin occupies 0x0010_0000..0x0018_b0c4.  Function
    // pointers are Thumb pointers, so the low bit must be set.  If a future
    // variant supplies another callback representation, do not jump through
    // it blindly; the synchronous success return still prevents the old
    // out-of-bounds table crash and the diagnostic above records the ABI.
    const INOTIA1_NATIVE_START: WIPICWord = 0x0010_0001;
    const INOTIA1_NATIVE_END: WIPICWord = 0x0018_b0c5;
    let callback_is_safe = (cb & 1) == 1
        && cb >= INOTIA1_NATIVE_START
        && cb < INOTIA1_NATIVE_END;

    if callback_is_safe {
        struct LegacySocketConnectCallback {
            fd: i32,
            cb: WIPICWord,
            param: WIPICWord,
        }

        #[async_trait::async_trait]
        impl MethodBody<WieError> for LegacySocketConnectCallback {
            #[tracing::instrument(name = "timer", skip_all)]
            async fn call(
                &self,
                context: &mut dyn WIPICContext,
                _: Box<[WIPICWord]>,
            ) -> Result<WIPICResult> {
                context.system().sleep(1).await;
                tracing::info!(
                    "[PHASE8_13_INOTIA1_NET30_CB] callback={:#010x} fd={} status=0 param={:#010x}",
                    self.cb,
                    self.fd,
                    self.param
                );
                context
                    .call_function(self.cb, &[self.fd as WIPICWord, 0, self.param])
                    .await?;
                Ok(WIPICResult { results: Vec::new() })
            }
        }

        context.spawn(Box::new(LegacySocketConnectCallback { fd, cb, param }))?;
    } else {
        tracing::warn!(
            "[PHASE8_13_INOTIA1_NET30] callback pointer not in expected Thumb native range; callback suppressed cb={cb:#010x}"
        );
    }

    Ok(0)
}

pub async fn socket_write(
    context: &mut dyn WIPICContext,
    fd: i32,
    ptr_buf: WIPICWord,
    len: i32,
) -> Result<i32> {
    if !is_inotia1_offline_network(context) {
        return Err(WieError::Unimplemented("4: MC_netSocketWrite".into()));
    }

    if len < 0 {
        tracing::warn!("[INOTIA1_CASH_NET] WRITE invalid negative len={len}");
        return Ok(-1);
    }

    let head_len = (len as usize).min(512);
    let mut head = vec![0u8; head_len];
    if head_len != 0 {
        context.read_bytes(ptr_buf, &mut head)?;
    }

    tracing::info!(
        "[PHASE8_12_CASH_TX] fd={fd} len={len} buf={ptr_buf:#010x} head={head:02x?} -> accepted locally"
    );

    // Phase 8.18 — respond only after the title itself emits its authentic
    // command-1 request. The recovered frame is 20 bytes and command byte 2
    // is 0x01. Preserve later commands for protocol capture instead of
    // guessing purchase semantics.
    let mut queued_reason: Option<&'static str> = None;
    if head.len() >= 3 && head[0] == 0x00 && head[1] == 0x14 && head[2] == 0x01 {
        queue_inotia1_cash_cmd1_success();
        queued_reason = Some("command1");
        tracing::info!(
            "[PHASE8_18_INOTIA1_CASH_INIT_RX] command=1 request accepted -> queued 28-byte local success response (common result=1)"
        );
    } else if head.len() >= 3 && head[2] == 0x05 {
        // Phase 8.40: classify the cash session only at this rare protocol
        // boundary. State 14 is the title's missing-prayer purchase prompt.
        // This replaces the old high-frequency diagnostics with three cheap
        // guest reads that occur only when the cash transfer starts.
        let detected_now = phase8_40_detect_emergency_prayer_cash(context, "command5");
        if detected_now {
            INOTIA1_EMERGENCY_PRAYER_CASH.store(true, Ordering::Relaxed);
        }
        let emergency_prayer = INOTIA1_EMERGENCY_PRAYER_CASH.load(Ordering::Relaxed);
        tracing::info!(
            "[PHASE8_40_INOTIA1_WIPE_CASH_LATCH] command5 detected_now={detected_now} latched={emergency_prayer}; reconnects cannot downgrade an active party-wipe flow"
        );
        // Phase 8.36: the broad heap/GOT name probe used here in Phase 8.35
        // was diagnostic-only and expensive.  Main-name recovery now happens
        // only at two exact native strlen callers, so command 5 stays purely
        // on the cash-shop protocol path.
        // Phase 8.24: command 5 requests the catalog transfer. Queue the empty
        // command-2 start; the reader automatically follows it with command 4
        // finalization so the original UI is not left in a half-transfer state.
        queue_inotia1_cash_cmd5_stage2();
        queued_reason = Some("command5-transfer");
        tracing::info!(
            "[PHASE8_24_INOTIA1_CASH_TRANSFER_SEQUENCE] outbound command=5 len={len} -> queued command-2 empty start + command-4 empty finalize"
        );
    } else if head.len() >= 3 && head[2] == 0x7b {
        // Phase 8.40: normal cash-shop exit keeps the Phase 8.34 cleanup. For
        // the party-wipe emergency shop only, first mirror the title's own
        // state-14 CLEAR branch so backing out cannot leave the outer death UI
        // parked in the purchase state after its network overlay disappears.
        let emergency_prayer = INOTIA1_EMERGENCY_PRAYER_CASH.load(Ordering::Relaxed);
        if emergency_prayer {
            phase8_40_restore_death_prompt_after_cash_cancel(context);
        }
        // Phase 8.34: static jump-table recovery proves command 123 has a real
        // server receive route. Replace any older pending bytes with the
        // native cleanup success frame and wake the reader so the title clears
        // its own cash-shop UI/description state.
        reset_inotia1_cash_session_wait();
        queue_inotia1_cmd123_cleanup_success();
        queued_reason = Some("command123-cleanup");
        tracing::info!(
            "[PHASE8_40_INOTIA1_CASH_EXIT_CLEANUP] outbound command=123 len={len} emergency_prayer={emergency_prayer} -> queued [00 04 7b 01] for native generic cleanup handler 0x001171fa"
        );
        INOTIA1_EMERGENCY_PRAYER_CASH.store(false, Ordering::Relaxed);
    } else if head.len() >= 3 && head[2] == 0x1e {
        let requested_page = head.last().copied().unwrap_or(0);
        // Phase 8.52 exposes one 11-record physical catalog below the title's
        // validated 12-record capacity. Any stale/nonzero page request receives
        // the same frame, so page navigation cannot hide items or expand the
        // catalog beyond the fixed table.
        trace_phase8_34_inotia1_page_globals(context, requested_page, "before-command30-single-page-response");
        queue_inotia1_cash_cmd30_page(0);
        queued_reason = Some("command30-complete-single-catalog");
        let emergency_prayer = INOTIA1_EMERGENCY_PRAYER_CASH.load(Ordering::Relaxed);
        let record_count = if emergency_prayer { 1 } else { 11 };
        tracing::info!(
            "[PHASE8_40_INOTIA1_CASH_CATALOG] outbound command=30 len={len} requested_page={requested_page} emergency_prayer={emergency_prayer} -> compatibility_header=[0,1] records={record_count} capacity=12 overflow=none all price=0"
        );
    } else if head.len() >= 3 && head[2] == 0x1f {
        // The first Phase 8.25 purchase attempt provided the complete authentic
        // request shape, including EUC-KR item-name bytes. A command-31 success
        // response is now safe to feed back through the same async reader wake.
        queue_inotia1_cash_cmd31_purchase_success();
        queued_reason = Some("command31-purchase");
        let emergency_prayer = INOTIA1_EMERGENCY_PRAYER_CASH.load(Ordering::Relaxed);
        tracing::info!(
            "[PHASE8_40_INOTIA1_CASH_PURCHASE] outbound command=31 len={len} emergency_prayer={emergency_prayer} head={head:02x?} -> queued local success balance=0"
        );
    } else if head.len() >= 3 && head[2] == 0x59 {
        // Phase 8.34 — a resource-exchange ticket already present in an older save
        // can still reach authentic command 89 because earlier compatibility gates
        // allow the item-use path. This resource belongs to the historical server
        // account, not the single-player inventory, so return common failure state=0
        // rather than the old synthetic success that misleadingly displayed '10 acquired'.
        queue_inotia1_cmd89_resource_exchange_reject();
        queued_reason = Some("command89-offline-reject");
        tracing::info!(
            "[PHASE8_34_INOTIA1_RESOURCE_EXCHANGE_OFFLINE_ONLY] outbound command=89 len={len} head={head:02x?} -> queued native failure state=0; server-account resources are not represented in single-player inventory"
        );
    } else if head.len() >= 3 {
        tracing::info!(
            "[PHASE8_18_INOTIA1_CASH_PROTOCOL] outbound command={} len={len}; no synthetic response yet",
            head[2]
        );
    }

    // Phase 8.25/8.26: command 30 and the subsequent command-31 purchase are
    // emitted after the guest has armed MC_netSetReadCB. Wake that stored
    // one-shot callback whenever local response bytes become available. The
    // same helper is safe for earlier responses; it is a no-op without a waiter.
    if let Some(reason) = queued_reason {
        wake_inotia1_cash_reader(context, reason)?;
    }

    // Pretend the complete buffer was accepted.  The original client then
    // advances its send cursor normally and waits for a read callback.
    Ok(len)
}

// Phase 8.16 — static disassembly plus the Phase 8.15 runtime trace resolves
// the two carrier-extension slots that follow the async connect entry:
//
//   interface + 0x7c (slot 31): SEND  (fd, source, remaining_length)
//   interface + 0x80 (slot 32): RECV  (fd, destination, remaining_length)
//
// Phase 8.15 accidentally routed slot 32 into the writer.  The first call had
// len=2 because the game was waiting for its server-first length header; after
// WIE claimed those two zero bytes were sent, the guest decoded a zero length
// and its next remaining count became -2.  Keep SEND as packet capture and
// make RECV serve only the minimal local command-0 greeting above.
pub async fn socket_write_ktf_legacy(
    context: &mut dyn WIPICContext,
    fd: i32,
    ptr_buf: WIPICWord,
    len: i32,
) -> Result<i32> {
    if !is_inotia1_offline_network(context) {
        return Err(WieError::Unimplemented(
            "31: KTF legacy MC_netSocketWrite".into(),
        ));
    }

    tracing::info!(
        "[PHASE8_16_INOTIA1_NET31_TX] fd={fd} buf={ptr_buf:#010x} len={len} -> offline packet capture"
    );
    socket_write(context, fd, ptr_buf, len).await
}

pub async fn socket_read_ktf_legacy(
    context: &mut dyn WIPICContext,
    fd: i32,
    ptr_buf: WIPICWord,
    len: i32,
) -> Result<i32> {
    if !is_inotia1_offline_network(context) {
        return Err(WieError::Unimplemented(
            "32: KTF legacy MC_netSocketRead".into(),
        ));
    }

    if len < 0 {
        tracing::warn!(
            "[PHASE8_16_INOTIA1_NET32_RX] fd={fd} invalid negative len={len}"
        );
        return Ok(-1);
    }
    if len == 0 {
        return Ok(0);
    }

    // Capture the guest's real protocol state at the command-1 receive
    // boundary. Read the local queue state in a short scope before taking the
    // mutable lock used to copy bytes.
    let trace_cmd1_state = {
        let rx = INOTIA1_CASH_RX_STATE.lock();
        rx.phase == CASH_RX_CMD1 && rx.offset == 0
    };
    if trace_cmd1_state {
        trace_inotia1_cash_protocol_state(context, "before-command1-response");
    }

    let catalog_page = *INOTIA1_CASH_CATALOG_PAGE.lock();
    let mut state = INOTIA1_CASH_RX_STATE.lock();

    // Phase 8.24: a zero-byte catalog transfer is a two-frame server sequence.
    // When command 2 has been completely consumed, advance directly to command
    // 4 on the next guest read instead of reporting WOULD_BLOCK in between.
    // This mirrors a continuous TCP stream while preserving the existing
    // callback/read behavior for all other phases.
    let frame: &[u8] = loop {
        let candidate: &[u8] = match state.phase {
            CASH_RX_HELLO => &INOTIA1_CASH_SERVER_HELLO,
            CASH_RX_CMD1 => &INOTIA1_CASH_CMD1_SUCCESS,
            CASH_RX_CMD5_STAGE2 => &INOTIA1_CASH_CMD5_STAGE2_EMPTY,
            CASH_RX_CMD5_STAGE4 => &INOTIA1_CASH_CMD5_STAGE4_FINALIZE_EMPTY,
            CASH_RX_CMD30_REENTRY => inotia1_cash_catalog_frame(catalog_page),
            CASH_RX_CMD31_PURCHASE => &INOTIA1_CASH_CMD31_PURCHASE_SUCCESS,
            CASH_RX_CMD89_RESOURCE_EXCHANGE => &INOTIA1_CMD89_RESOURCE_EXCHANGE_OFFLINE_REJECT,
            CASH_RX_CMD123_CLEANUP => &INOTIA1_CMD123_CLEANUP_SUCCESS,
            _ => {
                tracing::info!(
                    "[PHASE8_16_INOTIA1_NET32_RX] fd={fd} local response queue empty -> M_E_WOULDBLOCK ({M_E_WOULDBLOCK})"
                );
                return Ok(M_E_WOULDBLOCK);
            }
        };

        if state.offset < candidate.len() {
            break candidate;
        }

        if state.phase == CASH_RX_CMD5_STAGE2 {
            state.phase = CASH_RX_CMD5_STAGE4;
            state.offset = 0;
            tracing::info!(
                "[PHASE8_24_INOTIA1_CASH_TRANSFER_SEQUENCE] command-2 frame consumed -> command-4 empty finalize ready"
            );
            continue;
        }

        state.phase = CASH_RX_WAIT;
        state.offset = 0;
        tracing::info!(
            "[PHASE8_16_INOTIA1_NET32_RX] fd={fd} local frame consumed -> M_E_WOULDBLOCK ({M_E_WOULDBLOCK})"
        );
        return Ok(M_E_WOULDBLOCK);
    };

    let remaining = frame.len() - state.offset;
    let count = remaining.min(len as usize);
    let begin = state.offset;
    let end = begin + count;
    let bytes = &frame[begin..end];
    context.write_bytes(ptr_buf, bytes)?;
    state.offset = end;
    let phase = state.phase;
    let frame_len = frame.len();

    // Phase 8.27 — make the first cash-shop entry useful, not just the second.
    //
    // Field logs prove the client always completes command 2 + command 4, then
    // waits several seconds before its timer emits command 30.  That historical
    // network pacing is what leaves the first entry showing the old error/
    // disconnect UI even though the same session can later fetch the catalog.
    // Once command 4 is fully consumed, make the already-validated command-30
    // catalog immediately pending. The guest's existing MC_netSetReadCB path
    // will wake and dispatch it asynchronously; if the title later emits its
    // normal command-30 refresh, socket_write simply queues the same catalog
    // again. No UI state or save memory is patched directly.
    if phase == CASH_RX_CMD5_STAGE4 && end == frame_len {
        *INOTIA1_CASH_CATALOG_PAGE.lock() = 0;
        state.phase = CASH_RX_CMD30_REENTRY;
        state.offset = 0;
        let emergency_prayer = INOTIA1_EMERGENCY_PRAYER_CASH.load(Ordering::Relaxed);
        let record_count = if emergency_prayer { 1 } else { 11 };
        tracing::info!(
            "[PHASE8_40_INOTIA1_FIRST_OPEN_CATALOG] command-4 finalize consumed -> emergency_prayer={emergency_prayer} compatibility_header=[0,1] records={record_count} capacity=12 overflow=none catalog immediately pending"
        );
    }

    // Phase 8.40 — successful emergency-prayer purchase handoff.
    //
    // The Phase 8.39 field log proves command 31 succeeds and the title
    // persists four additional save bytes, then its native outer state becomes
    // 1. Our old emergency CLEAR hook was still latched and overwrote that
    // legitimate post-purchase state with 11, leaving the cash overlay and
    // death state inconsistent. The same log also proves the guest re-arms
    // MC_netSetReadCB immediately after the save commit. Therefore, once the
    // complete command-31 success frame has been delivered, retire the
    // cancellation latch and leave command-123 cleanup pending. The re-armed
    // callback will consume it immediately, while the original command-31
    // handler remains free to drive its own state-13/state-1 revival path.
    if phase == CASH_RX_CMD31_PURCHASE && end == frame_len {
        let emergency_prayer = INOTIA1_EMERGENCY_PRAYER_CASH.swap(false, Ordering::Relaxed);
        if emergency_prayer {
            state.phase = CASH_RX_CMD123_CLEANUP;
            state.offset = 0;
            tracing::info!(
                "[PHASE8_40_INOTIA1_PRAYER_PURCHASE_HANDOFF] command-31 success fully delivered -> emergency cancellation latch cleared; command-123 cleanup queued for the guest's immediate read-callback re-arm; native state13/state1 return preserved"
            );
        }
    }

    tracing::info!(
        "[PHASE8_16_INOTIA1_NET32_RX] fd={fd} phase={phase} buf={ptr_buf:#010x} requested={len} returned={count} offset={end}/{frame_len} bytes={bytes:02x?}"
    );
    Ok(count as i32)
}

pub async fn socket_read(
    context: &mut dyn WIPICContext,
    fd: i32,
    ptr_buf: WIPICWord,
    len: i32,
) -> Result<i32> {
    if !is_inotia1_offline_network(context) {
        return Err(WieError::Unimplemented("5: MC_netSocketRead".into()));
    }

    tracing::info!(
        "[INOTIA1_CASH_NET] MC_netSocketRead fd={fd} buf={ptr_buf:#010x} len={len} -> M_E_WOULDBLOCK ({M_E_WOULDBLOCK})"
    );
    Ok(M_E_WOULDBLOCK)
}

pub async fn socket_close(context: &mut dyn WIPICContext, fd: i32) -> Result<i32> {
    if is_inotia1_offline_network(context) {
        // Phase 8.19 — the earlier 27-byte experimental frame was structurally consumed but
        // the native client rejects one of its semantics before issuing the
        // next shop request.  Record the exact guest call site *before* reset
        // so the next field test tells us which native error branch closed the
        // socket.  This is diagnostic only and does not weaken save handling.
        trace_inotia1_cash_reject(context, "MC_netSocketClose", Some(fd));
        reset_inotia1_cash_rx();
        reset_inotia1_cash_read_callback();
        let emergency_prayer = INOTIA1_EMERGENCY_PRAYER_CASH.load(Ordering::Relaxed);
        tracing::info!(
            "[PHASE8_40_INOTIA1_WIPE_CASH_LATCH] socket close preserves emergency latch={emergency_prayer}"
        );
        tracing::info!("[INOTIA1_CASH_NET] MC_netSocketClose({fd}) -> 0");
        return Ok(0);
    }

    tracing::warn!("stub MC_netSocketClose({fd})");
    Ok(-1) // M_E_ERROR
}

pub async fn set_read_cb(
    context: &mut dyn WIPICContext,
    fd: i32,
    cb: WIPICWord,
    param: WIPICWord,
) -> Result<i32> {
    if is_inotia1_offline_network(context) {
        *INOTIA1_CASH_READ_CALLBACK.lock() = Inotia1CashReadCallback {
            active: true,
            fd,
            cb,
            param,
        };
        tracing::info!(
            "[PHASE8_25_INOTIA1_CASH_READ_CALLBACK] MC_netSetReadCB fd={fd} cb={cb:#010x} param={param:#010x} persisted; awaiting local protocol response"
        );

        // Close the race where local bytes were queued just before the guest
        // registered its callback. If a frame is already pending, schedule the
        // same asynchronous data-ready callback immediately.
        wake_inotia1_cash_reader(context, "register-with-pending-data")?;
        return Ok(0);
    }

    Err(WieError::Unimplemented("13: MC_netSetReadCB".into()))
}

pub async fn set_write_cb(
    context: &mut dyn WIPICContext,
    fd: i32,
    cb: WIPICWord,
    param: WIPICWord,
) -> Result<i32> {
    if is_inotia1_offline_network(context) {
        tracing::info!(
            "[INOTIA1_CASH_NET] MC_netSetWriteCB fd={fd} cb={cb:#010x} param={param:#010x} -> 0"
        );
        return Ok(0);
    }

    Err(WieError::Unimplemented("14: MC_netSetWriteCB".into()))
}
