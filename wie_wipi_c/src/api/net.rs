use alloc::{boxed::Box, vec::Vec};

use wipi_types::wipic::WIPICWord;

use wie_util::{Result, WieError};

use crate::{WIPICResult, context::WIPICContext, method::MethodBody};

/// Legacy WIPI network-session compatibility.
///
/// Original feature phones asked the carrier runtime to establish a packet-data
/// session before a game could perform its own networking. The old WIE stub
/// always called the completion callback with M_E_ERROR, which makes many
/// preserved games stop at obsolete carrier/internet-verification screens.
///
/// We emulate *session establishment only*: the callback reports M_E_SUCCESS.
/// This does not fabricate HTTP/socket payloads or contact a verification
/// server. Games that require real protocol traffic will continue into their
/// next network API, which can then be traced and implemented independently.
pub async fn connect(context: &mut dyn WIPICContext, cb: WIPICWord, param: WIPICWord) -> Result<i32> {
    tracing::info!("[NET_COMPAT] MC_netConnect({cb:#x}, {param:#x}) -> scheduling legacy session success");

    struct ConnectCallback {
        cb: WIPICWord,
        param: WIPICWord,
    }

    #[async_trait::async_trait]
    impl MethodBody<WieError> for ConnectCallback {
        #[tracing::instrument(name = "timer", skip_all)]
        async fn call(&self, context: &mut dyn WIPICContext, _: Box<[WIPICWord]>) -> Result<WIPICResult> {
            // Keep a short asynchronous delay so titles that expect the carrier
            // connection callback on a later event/timer tick retain that flow.
            context.system().sleep(1).await;

            // WIPI success is 0. The previous implementation used u32::MAX
            // (M_E_ERROR), which made verification-gated titles stop here.
            tracing::info!("[NET_COMPAT] MC_netConnect callback status=M_E_SUCCESS(0) param={:#x}", self.param);
            context.call_function(self.cb, &[0, self.param]).await?;

            Ok(WIPICResult { results: Vec::new() })
        }
    }

    context.spawn(Box::new(ConnectCallback { cb, param }))?;

    // Request accepted; completion is delivered asynchronously above.
    Ok(0)
}

pub async fn close(_context: &mut dyn WIPICContext) -> Result<()> {
    tracing::info!("[NET_COMPAT] MC_netClose() -> success");

    Ok(())
}

pub async fn socket_close(_context: &mut dyn WIPICContext, fd: i32) -> Result<i32> {
    // Closing a synthetic/unsupported legacy socket should be idempotent from
    // the guest's perspective. Returning an error here can send otherwise
    // tolerant games into their network-failure UI during cleanup.
    tracing::info!("[NET_COMPAT] MC_netSocketClose({fd}) -> success");

    Ok(0)
}
