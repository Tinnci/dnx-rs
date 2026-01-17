//! Control flow handlers (reset, halt, done).

use crate::events::{DnxEvent, DnxObserver, LogLevel};
use crate::transport::UsbTransport;
use anyhow::Result;
use tracing::info;

use super::{HandleResult, HandlerContext};

/// RESET - GPP Reset.
pub fn handle_reset<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("RESET: Device will reset");
    ctx.log(LogLevel::Info, "Received RESET - device will re-enumerate");
    ctx.state.fw_done = true;
    ctx.state.gpp_reset = true;
    Ok(HandleResult::NeedReEnumerate)
}

/// HLT$ - Update Successful.
pub fn handle_hlt_success<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("HLT$: Firmware update successful");
    ctx.log(LogLevel::Info, "Firmware update successful");
    ctx.state.fw_done = true;
    ctx.state.ifwi_done = true;
    Ok(HandleResult::FwDone)
}

/// HLT0 - FW size is 0.
pub fn handle_hlt0<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    tracing::warn!("HLT0: FW file has no size");
    ctx.log(LogLevel::Warn, "DnX FW or IFWI size is 0");
    ctx.state.fw_done = true;
    Ok(HandleResult::FwDone)
}

/// DONE - All complete.
pub fn handle_done<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("DONE: All operations complete");
    ctx.log(LogLevel::Info, "All operations complete");
    ctx.state.os_done = true;
    ctx.emit(DnxEvent::Complete);
    Ok(HandleResult::Complete)
}
