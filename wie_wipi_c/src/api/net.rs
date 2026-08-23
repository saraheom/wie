use alloc::{boxed::Box, vec, vec::Vec};

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

fn is_inotia1_offline_network(context: &mut dyn WIPICContext) -> bool {
    let system = context.system();
    system.aid() == INOTIA1_AID && system.pid() == INOTIA1_PID
}

pub async fn connect(context: &mut dyn WIPICContext, cb: WIPICWord, param: WIPICWord) -> Result<i32> {
    let inotia1 = is_inotia1_offline_network(context);

    if inotia1 {
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

    let head_len = (len as usize).min(128);
    let mut head = vec![0u8; head_len];
    if head_len != 0 {
        context.read_bytes(ptr_buf, &mut head)?;
    }

    tracing::info!(
        "[PHASE8_12_CASH_TX] fd={fd} len={len} buf={ptr_buf:#010x} head={head:02x?} -> accepted locally"
    );

    // Pretend the complete buffer was accepted.  The original client then
    // advances its send cursor normally and waits for a read callback.
    Ok(len)
}

// Phase 8.15 — KTF Inotia 1 uses another carrier-extension entry at
// interface offset 0x80 (slot 32) immediately after the asynchronous slot-30
// connect callback succeeds. Static analysis of PD005362 shows a three-word
// call shape `(fd, buffer, length)` and the same return convention as a socket
// write: positive byte count means progress and M_E_WOULDBLOCK is retryable.
//
// Route only this legacy extension through the already-isolated offline
// packet-capture writer. This keeps the old server unreachable while allowing
// the original game to emit the request bytes needed to reconstruct the cash
// shop protocol locally.
pub async fn socket_write_ktf_legacy(
    context: &mut dyn WIPICContext,
    fd: i32,
    ptr_buf: WIPICWord,
    len: i32,
) -> Result<i32> {
    if !is_inotia1_offline_network(context) {
        return Err(WieError::Unimplemented(
            "32: KTF legacy MC_netSocketWrite".into(),
        ));
    }

    tracing::info!(
        "[PHASE8_15_INOTIA1_NET32] fd={fd} buf={ptr_buf:#010x} len={len} -> offline packet-capture writer"
    );
    socket_write(context, fd, ptr_buf, len).await
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
