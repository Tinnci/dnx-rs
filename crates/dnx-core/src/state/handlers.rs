//! ACK handlers - dispatch logic for each ACK code.

use crate::events::{DnxEvent, DnxObserver, LogLevel};
use crate::protocol::AckCode;
use crate::protocol::constants::*;
use crate::state::machine::{DldrState, StateMachineContext};
use crate::transport::UsbTransport;
use anyhow::Result;
use tracing::{debug, info, warn};

/// Result of handling an ACK.
#[derive(Debug)]
pub enum HandleResult {
    /// Continue processing.
    Continue,
    /// FW download complete, device will reset.
    FwDone,
    /// OS download complete.
    OsDone,
    /// All operations complete.
    Complete,
    /// Error occurred.
    Error(String),
    /// Device disconnected, need to re-enumerate.
    NeedReEnumerate,
}

/// ACK handler context containing all resources.
pub struct HandlerContext<'a, T: UsbTransport, O: DnxObserver> {
    pub transport: &'a T,
    pub observer: &'a O,
    pub state: &'a mut StateMachineContext,
    /// FW DnX binary data.
    pub fw_dnx_data: Option<&'a [u8]>,
    /// FW image data (IFWI).
    pub fw_image_data: Option<&'a [u8]>,
    /// OS DnX binary data.
    pub os_dnx_data: Option<&'a [u8]>,
    /// OS image data.
    pub os_image_data: Option<&'a [u8]>,
}

impl<'a, T: UsbTransport, O: DnxObserver> HandlerContext<'a, T, O> {
    fn emit(&self, event: DnxEvent) {
        self.observer.on_event(&event);
    }

    fn log(&self, level: LogLevel, message: impl Into<String>) {
        self.emit(DnxEvent::Log {
            level,
            message: message.into(),
        });
    }
}

/// Handle an ACK code and perform the appropriate action.
pub fn handle_ack<T: UsbTransport, O: DnxObserver>(
    ack: &AckCode,
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    ctx.emit(DnxEvent::AckReceived {
        ack: ack.as_ascii(),
    });

    // First check for error codes
    if ack.is_error() {
        let msg = format!("Device error: {}", ack.as_ascii());
        ctx.emit(DnxEvent::Error {
            code: ack.value() as u32,
            message: msg.clone(),
        });
        return Ok(HandleResult::Error(msg));
    }

    // Match 4-byte ACKs
    if ack.matches_u32(BULK_ACK_DFRM) {
        return handle_dfrm(ctx);
    }
    if ack.matches_u32(BULK_ACK_DxxM) {
        return handle_dxxm(ctx);
    }
    if ack.matches_u32(BULK_ACK_DXBL) {
        return handle_dxbl(ctx);
    }
    if ack.matches_u32(BULK_ACK_READY_UPH) {
        return handle_ruph(ctx);
    }
    if ack.matches_u32(BULK_ACK_DMIP) {
        return handle_dmip(ctx);
    }
    if ack.matches_u32(BULK_ACK_LOFW) {
        return handle_lofw(ctx);
    }
    if ack.matches_u32(BULK_ACK_HIFW) {
        return handle_hifw(ctx);
    }
    if ack.matches_u32(BULK_ACK_SSFW) {
        return handle_ssfw(ctx);
    }
    if ack.matches_u32(BULK_ACK_UPDATE_SUCCESSFUL) {
        return handle_hlt_success(ctx);
    }
    if ack.matches_u32(BULK_ACK_HLT0) {
        return handle_hlt0(ctx);
    }
    if ack.matches_u32(BULK_ACK_DONE) {
        return handle_done(ctx);
    }
    if ack.matches_u32(BULK_ACK_DORM) {
        return handle_dorm(ctx);
    }
    if ack.matches_u32(BULK_ACK_RIMG) {
        return handle_rimg(ctx);
    }
    if ack.matches_u32(BULK_ACK_EOIU) {
        return handle_eoiu(ctx);
    }

    // Match 5+ byte ACKs
    if ack.matches_u64(BULK_ACK_READY_UPH_SIZE) {
        return handle_ruphs(ctx);
    }
    if ack.matches_u64(BULK_ACK_GPP_RESET) {
        return handle_reset(ctx);
    }
    if ack.matches_u64(BULK_ACK_PSFW1) {
        return handle_psfw1(ctx);
    }
    if ack.matches_u64(BULK_ACK_PSFW2) {
        return handle_psfw2(ctx);
    }
    if ack.matches_u64(BULK_ACK_VEDFW) {
        return handle_vedfw(ctx);
    }
    if ack.matches_u64(BULK_ACK_ROSIP) {
        return handle_rosip(ctx);
    }

    // Unknown ACK
    warn!(ack = %ack.as_ascii(), "Unhandled ACK code");
    ctx.log(LogLevel::Warn, format!("Unhandled ACK: {}", ack.as_ascii()));
    Ok(HandleResult::Continue)
}

// ============================================================================
// Individual ACK Handlers
// ============================================================================

/// DFRM - Virgin part DnX.
fn handle_dfrm<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("DFRM: Virgin part detected, starting FW download");
    ctx.log(
        LogLevel::Info,
        "Virgin part detected - starting firmware download",
    );

    if ctx.state.ifwi_wipe_enable {
        ctx.log(LogLevel::Info, "EMMC is virgin, no need to wipe IFWI");
        ctx.state.fw_done = true;
        ctx.state.ifwi_done = false;
        return Ok(HandleResult::FwDone);
    }

    ctx.state.goto_state(DldrState::FwNormal);
    Ok(HandleResult::Continue)
}

/// DxxM - Non-virgin part DnX.
fn handle_dxxm<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("DxxM: Non-virgin part detected");
    ctx.log(LogLevel::Info, "Non-virgin part detected");

    let is_dnx_os = (ctx.state.gp_flags & 0x20) != 0;

    if ctx.state.ifwi_wipe_enable {
        ctx.state.goto_state(DldrState::FwWipe);
    } else if is_dnx_os {
        ctx.state.goto_state(DldrState::FwMisc);
    } else {
        ctx.state.goto_state(DldrState::FwNormal);
    }

    Ok(HandleResult::Continue)
}

/// DXBL - Download Execute Bootloader.
fn handle_dxbl<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("DXBL: Sending DnX binary");
    ctx.log(LogLevel::Info, "Sending DnX binary");

    let data = if ctx.state.state.is_fw() {
        ctx.fw_dnx_data
    } else {
        ctx.os_dnx_data
    };

    if let Some(dnx_data) = data {
        ctx.transport.write(dnx_data)?;
        ctx.emit(DnxEvent::Progress {
            phase: crate::events::DnxPhase::FirmwareDownload,
            operation: "DnX binary".to_string(),
            current: dnx_data.len() as u64,
            total: dnx_data.len() as u64,
        });
    } else {
        warn!("No DnX data available for current state");
        ctx.log(LogLevel::Warn, "No DnX data available");
    }

    Ok(HandleResult::Continue)
}

/// RUPHS - Ready for Update Profile Header Size.
fn handle_ruphs<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("RUPHS: Sending FW Update Profile Header Size");
    ctx.log(LogLevel::Debug, "Sending FW Update Profile Header Size");

    // TODO: Send actual header size from FW image
    // For now, send placeholder
    let header_size: u32 = crate::protocol::constants::D0_FW_UPDATE_PROFILE_HDR_SIZE as u32;
    ctx.transport.write(&header_size.to_le_bytes())?;

    Ok(HandleResult::Continue)
}

/// RUPH - Ready for Update Profile Header.
fn handle_ruph<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("RUPH: Sending FW Update Profile Header");
    ctx.log(LogLevel::Debug, "Sending FW Update Profile Header");

    // TODO: Extract and send actual profile header from FW image
    // For now, this is a placeholder

    Ok(HandleResult::Continue)
}

/// DMIP - Download MIP.
fn handle_dmip<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("DMIP: Sending MIP (Module Info Pointer)");
    ctx.log(LogLevel::Debug, "Sending MIP");
    // TODO: Send MIP data from FW image
    Ok(HandleResult::Continue)
}

/// LOFW - Low FW (first 128KB).
fn handle_lofw<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("LOFW: Sending first 128KB FW chunk");
    ctx.log(LogLevel::Debug, "Sending first 128KB chunk");
    // TODO: Send first 128KB of FW image
    Ok(HandleResult::Continue)
}

/// HIFW - High FW (second 128KB).
fn handle_hifw<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("HIFW: Sending second 128KB FW chunk");
    ctx.log(LogLevel::Debug, "Sending second 128KB chunk");
    // TODO: Send second 128KB of FW image
    Ok(HandleResult::Continue)
}

/// PSFW1 - Primary Security FW 1.
fn handle_psfw1<T: UsbTransport, O: DnxObserver>(
    _ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    debug!("PSFW1: Sending Primary Security FW 1 chunk");
    // TODO: Implement chunked sending with tracker
    Ok(HandleResult::Continue)
}

/// PSFW2 - Primary Security FW 2.
fn handle_psfw2<T: UsbTransport, O: DnxObserver>(
    _ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    debug!("PSFW2: Sending Primary Security FW 2 chunk");
    // TODO: Implement chunked sending with tracker
    Ok(HandleResult::Continue)
}

/// SSFW - Secondary Security FW.
fn handle_ssfw<T: UsbTransport, O: DnxObserver>(
    _ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    debug!("SSFW: Sending Secondary Security FW");
    // TODO: Send SSFW data
    Ok(HandleResult::Continue)
}

/// VEDFW - Video Encoder/Decoder FW.
fn handle_vedfw<T: UsbTransport, O: DnxObserver>(
    _ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    debug!("VEDFW: Sending Video Encoder/Decoder FW");
    // TODO: Send VEDFW data
    Ok(HandleResult::Continue)
}

/// RESET - GPP Reset.
fn handle_reset<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("RESET: Device will reset");
    ctx.log(LogLevel::Info, "Received RESET - device will re-enumerate");
    ctx.state.fw_done = true;
    ctx.state.gpp_reset = true;
    Ok(HandleResult::NeedReEnumerate)
}

/// HLT$ - Update Successful.
fn handle_hlt_success<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("HLT$: Firmware update successful");
    ctx.log(LogLevel::Info, "Firmware update successful");
    ctx.state.fw_done = true;
    ctx.state.ifwi_done = true;
    Ok(HandleResult::FwDone)
}

/// HLT0 - FW size is 0.
fn handle_hlt0<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    warn!("HLT0: FW file has no size");
    ctx.log(LogLevel::Warn, "DnX FW or IFWI size is 0");
    ctx.state.fw_done = true;
    Ok(HandleResult::FwDone)
}

/// DORM - OS Recovery Mode.
fn handle_dorm<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("DORM: Entering OS Recovery mode");
    ctx.log(LogLevel::Info, "Entering OS Recovery mode");
    ctx.state.goto_state(DldrState::OsNormal);
    Ok(HandleResult::Continue)
}

/// ROSIP - Ready for OSIP.
fn handle_rosip<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("ROSIP: Sending OSIP data");
    ctx.log(LogLevel::Debug, "Sending OSIP partition table");
    // TODO: Send OSIP data from OS image
    Ok(HandleResult::Continue)
}

/// RIMG - Request Image chunk.
fn handle_rimg<T: UsbTransport, O: DnxObserver>(
    _ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    debug!("RIMG: Sending OS image chunk");
    // TODO: Implement OS image chunked sending
    Ok(HandleResult::Continue)
}

/// EOIU - End of Image Update.
fn handle_eoiu<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("EOIU: OS image transfer complete");
    ctx.log(LogLevel::Info, "OS image transfer complete");
    Ok(HandleResult::Continue)
}

/// DONE - All complete.
fn handle_done<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("DONE: All operations complete");
    ctx.log(LogLevel::Info, "All operations complete");
    ctx.state.os_done = true;
    ctx.emit(DnxEvent::Complete);
    Ok(HandleResult::Complete)
}
