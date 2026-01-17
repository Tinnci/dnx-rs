//! ACK handlers - dispatch logic for each ACK code.
//!
//! This module is split into submodules by functionality:
//! - `chaabi`: Chaabi firmware helper functions
//! - `control`: Control flow handlers (reset, halt, done)
//! - `firmware`: Firmware download handlers
//! - `os`: OS download handlers
//! - `security`: Security firmware handlers

mod chaabi;
mod control;
mod firmware;
mod os;
mod security;

use crate::events::{DnxEvent, DnxObserver, LogLevel};
use crate::protocol::AckCode;
use crate::protocol::constants::*;
use crate::state::machine::StateMachineContext;
use crate::transport::UsbTransport;
use anyhow::Result;
use tracing::warn;

// Re-export submodule handlers for internal use
use control::{handle_done, handle_hlt_success, handle_hlt0, handle_reset};
use firmware::{
    handle_dcfi00, handle_dfrm, handle_difwi, handle_dmip, handle_dxbl, handle_dxxm, handle_hifw,
    handle_lofw, handle_ruph, handle_ruphs,
};
use os::{handle_dorm, handle_eoiu, handle_rimg, handle_rosip};
use security::{handle_psfw1, handle_psfw2, handle_ssfw, handle_vedfw};

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
    /// Parsed FW image.
    pub fw_image: Option<&'a crate::payload::FirmwareImage>,
    /// OS DnX binary data.
    pub os_dnx_data: Option<&'a [u8]>,
    /// Parsed OS image.
    pub os_image: Option<&'a crate::payload::OsImage>,
}

impl<'a, T: UsbTransport, O: DnxObserver> HandlerContext<'a, T, O> {
    pub(crate) fn emit(&self, event: DnxEvent) {
        self.observer.on_event(&event);
    }

    pub(crate) fn log(&self, level: LogLevel, message: impl Into<String>) {
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

    // Match 5+ byte ACKs first (to avoid prefix collisions with 4-byte ones)
    if ack.matches_u64(BULK_ACK_READY_UPH_SIZE) {
        return handle_ruphs(ctx);
    }
    if ack.matches_u64(BULK_ACK_DCFI00) {
        return handle_dcfi00(ctx);
    }
    if ack.matches_u64(BULK_ACK_DIFWI) {
        return handle_difwi(ctx);
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
    if ack.matches_u64(BULK_ACK_OSIPSZ) {
        // Just log it for now
        ctx.log(LogLevel::Debug, "Received OSIP Sz request");
        return Ok(HandleResult::Continue);
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

    // Unknown ACK
    warn!(ack = %ack.as_ascii(), "Unhandled ACK code");
    ctx.log(LogLevel::Warn, format!("Unhandled ACK: {}", ack.as_ascii()));
    Ok(HandleResult::Continue)
}
