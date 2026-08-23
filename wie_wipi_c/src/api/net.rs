use alloc::{boxed::Box, vec, vec::Vec};

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

// Phase 8.24 — retry/re-entry probe. After the user leaves the first cash-shop
// attempt, the original title sends command 123 (one-way reset/cancel) followed
// by command 30 carrying the handset identifier. Command 30 has a dedicated
// receive handler. Its success path begins by consuming three one-byte fields;
// a zero-count third field avoids the variable-length record loop while still
// allowing the title to execute its original completion/UI state transition.
// This is deliberately metadata-only: no item or purchase record is fabricated.
const INOTIA1_CASH_CMD30_REENTRY_EMPTY: [u8; 7] = [
    0x00, 0x07, 0x1e, // length=7, server command=30
    0x01, // common result/state = success
    0x00, 0x00, 0x00, // three command-30 fixed fields; record count = 0
];

const CASH_RX_HELLO: u8 = 0;
const CASH_RX_WAIT: u8 = 1;
const CASH_RX_CMD1: u8 = 2;
const CASH_RX_CMD5_STAGE2: u8 = 3;
const CASH_RX_CMD5_STAGE4: u8 = 4;
const CASH_RX_CMD30_REENTRY: u8 = 5;

#[derive(Copy, Clone)]
struct Inotia1CashRxState {
    phase: u8,
    offset: usize,
}

static INOTIA1_CASH_RX_STATE: Mutex<Inotia1CashRxState> = Mutex::new(Inotia1CashRxState {
    phase: CASH_RX_HELLO,
    offset: 0,
});

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

fn queue_inotia1_cash_cmd30_reentry() {
    *INOTIA1_CASH_RX_STATE.lock() = Inotia1CashRxState {
        phase: CASH_RX_CMD30_REENTRY,
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
    if head.len() >= 3 && head[0] == 0x00 && head[1] == 0x14 && head[2] == 0x01 {
        queue_inotia1_cash_cmd1_success();
        tracing::info!(
            "[PHASE8_18_INOTIA1_CASH_INIT_RX] command=1 request accepted -> queued 28-byte local success response (common result=1)"
        );
    } else if head.len() >= 3 && head[2] == 0x05 {
        // Phase 8.24: command 5 requests the catalog transfer. Queue the empty
        // command-2 start; the reader automatically follows it with command 4
        // finalization so the original UI is not left in a half-transfer state.
        queue_inotia1_cash_cmd5_stage2();
        tracing::info!(
            "[PHASE8_24_INOTIA1_CASH_TRANSFER_SEQUENCE] outbound command=5 len={len} -> queued command-2 empty start + command-4 empty finalize"
        );
    } else if head.len() >= 3 && head[2] == 0x7b {
        // The observed 00 04 7b 00 packet is emitted when leaving/re-entering
        // the shop and has no matching receive-dispatch handler. Treat it as
        // the client's one-way session reset/cancel marker and clear only our
        // pending local-response state.
        reset_inotia1_cash_session_wait();
        tracing::info!(
            "[PHASE8_24_INOTIA1_CASH_REENTRY] outbound command=123 len={len} -> local session queue reset; no response"
        );
    } else if head.len() >= 3 && head[2] == 0x1e {
        queue_inotia1_cash_cmd30_reentry();
        tracing::info!(
            "[PHASE8_24_INOTIA1_CASH_REENTRY] outbound command=30 len={len} -> queued minimal command-30 success/zero-record response"
        );
    } else if head.len() >= 3 {
        tracing::info!(
            "[PHASE8_18_INOTIA1_CASH_PROTOCOL] outbound command={} len={len}; no synthetic response yet",
            head[2]
        );
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
            CASH_RX_CMD30_REENTRY => &INOTIA1_CASH_CMD30_REENTRY_EMPTY,
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
        tracing::info!(
            "[PHASE8_12_CASH_RX_WAIT] MC_netSetReadCB fd={fd} cb={cb:#010x} param={param:#010x} registered; awaiting local protocol response"
        );
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
