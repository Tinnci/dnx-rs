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
    /// Parsed FW image.
    pub fw_image: Option<&'a crate::payload::FirmwareImage>,
    /// OS DnX binary data.
    pub os_dnx_data: Option<&'a [u8]>,
    /// Parsed OS image.
    pub os_image: Option<&'a crate::payload::OsImage>,
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

    ctx.emit(DnxEvent::PhaseChanged {
        from: crate::events::DnxPhase::Handshake,
        to: crate::events::DnxPhase::FirmwareDownload,
    });
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

    ctx.emit(DnxEvent::PhaseChanged {
        from: crate::events::DnxPhase::Handshake,
        to: crate::events::DnxPhase::FirmwareDownload,
    });

    // Critical fix for Non-Virgin devices (like Z3580 Moorefield):
    // Based on xFSTK's EmmcFW::InitDnxHdr logic, the device expects a dynamic 24-byte header:
    // [0..4]   - File Size (u32 LE)
    // [4..8]   - GP Flags (u32 LE)
    // [8..20]  - Reserved (3x u32 LE, all 0)
    // [20..24] - Checksum (u32 LE) = File Size ^ GP Flags
    if let Some(dnx_data) = ctx.fw_dnx_data {
        let file_size = dnx_data.len() as u32;
        let gp_flags = ctx.state.gp_flags;
        let checksum = file_size ^ gp_flags;

        let mut header = [0u8; 24];
        header[0..4].copy_from_slice(&file_size.to_le_bytes());
        header[4..8].copy_from_slice(&gp_flags.to_le_bytes());
        // 8..12, 12..16, 16..20 are 0 (already zeroed by initiation)
        header[20..24].copy_from_slice(&checksum.to_le_bytes());

        info!(
            "DxxM: Sending dynamic DnX header (Size: {}, GP: 0x{:08X}, CS: 0x{:08X})",
            file_size, gp_flags, checksum
        );
        ctx.transport.write(&header)?;
    } else {
        warn!("DxxM: No FW DnX data available to construct header!");
    }

    Ok(HandleResult::Continue)
}

/// DCFI00 - Download Chaabi Firmware Image.
fn handle_dcfi00<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("DCFI00: Device requested Chaabi Firmware");
    ctx.log(LogLevel::Info, "Device requested Chaabi FW (DCFI00)");

    if let Some(dnx_data) = ctx.fw_dnx_data {
        // Use build_chaabi_payload which constructs: [CDPH Header] + [Token + FW]
        if let Some(chaabi_payload) = build_chaabi_payload(dnx_data) {
            info!("Built Chaabi FW payload: {} bytes", chaabi_payload.len());
            ctx.log(
                LogLevel::Info,
                format!("Sending Chaabi FW: {} bytes", chaabi_payload.len()),
            );
            ctx.transport.write(&chaabi_payload)?;
            ctx.emit(DnxEvent::Progress {
                phase: crate::events::DnxPhase::FirmwareDownload,
                operation: "Chaabi FW".to_string(),
                current: chaabi_payload.len() as u64,
                total: chaabi_payload.len() as u64,
            });
            debug!("Sent Chaabi FW");

            // Prepare IFWI state for next phase
            // IFWI is everything BEFORE the Token+FW section.
            // Use find_chaabi_range to get the start offset.
            if let Some((chaabi_start, _)) = find_chaabi_range(dnx_data) {
                let ifwi_len = chaabi_start;
                ctx.state.ifwi_state =
                    crate::payload::ChunkState::new(ifwi_len, crate::protocol::constants::ONE28_K);
                info!(
                    "Prepared IFWI state: size={} chunks={}",
                    ifwi_len, ctx.state.ifwi_state.total
                );
            }
        } else {
            let msg = "Failed to find Chaabi (CHFI) section in firmware file!";
            warn!("{}", msg);
            ctx.log(LogLevel::Error, msg);
            // Returning Error to stop the process as this is critical
            return Ok(HandleResult::Error(msg.to_string()));
        }
    } else {
        warn!("DCFI00: No FW data available!");
        ctx.log(LogLevel::Warn, "No FW data available for DCFI00");
    }

    Ok(HandleResult::Continue)
}

/// DIFWI - Download Integrated Firmware Image.
fn handle_difwi<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    debug!("DIFWI: Device requested IFWI chunk");

    // We assume IFWI data corresponds to 0..chaabi_start in the dnx_fwr.bin
    // This state should have been initialized in handle_dcfi00 success path.
    // If not, we try to initialize it here or fail.

    if ctx.state.ifwi_state.total == 0 {
        // Not initialized? Try to find boundaries again.
        if let Some(dnx_data) = ctx.fw_dnx_data {
            if let Some((start, _)) = find_chaabi_range(dnx_data) {
                let ifwi_len = start;
                ctx.state.ifwi_state =
                    crate::payload::ChunkState::new(ifwi_len, crate::protocol::constants::ONE28_K);
            }
        }
    }

    if let Some(dnx_data) = ctx.fw_dnx_data {
        // IFWI data is [0 .. ifwi_state.total_size]
        // But ChunkState doesn't store total_size directly in a way we can slice original data easily
        // Wait, ChunkState stores total (chunks). Not bytes.
        // Actually ChunkState stores: current (chunk index), total (chunks).
        // It does NOT store the data source offset.
        // We typically use the `next_chunk` method which slices the input buffer based on internal state.

        // However, `next_chunk` expects the *specific payload/buffer* as input.
        // If IFWI is a slice of dnx_data, we need that slice.
        // Re-calculate the range or store it?

        // Efficient way: re-find range (it's fast)
        if let Some((chaabi_start, _)) = find_chaabi_range(dnx_data) {
            let ifwi_data = &dnx_data[0..chaabi_start];

            if let Some(chunk) = ctx.state.ifwi_state.next_chunk(ifwi_data) {
                ctx.transport.write(chunk)?;
                ctx.emit(DnxEvent::Progress {
                    phase: crate::events::DnxPhase::FirmwareDownload,
                    operation: "IFWI".to_string(),
                    current: ctx.state.ifwi_state.current as u64,
                    total: ctx.state.ifwi_state.total as u64,
                });
                info!(
                    "Sent IFWI chunk {}/{}: {} bytes",
                    ctx.state.ifwi_state.current,
                    ctx.state.ifwi_state.total,
                    chunk.len()
                );
            } else {
                warn!("DIFWI: No more chunks to send (or completed)");
            }
        } else {
            warn!("DIFWI: Could not determine IFWI range");
        }
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

    if let Some(fw) = ctx.fw_image {
        let size_bytes = fw.profile_header_size_bytes();
        ctx.transport.write(&size_bytes)?;
        debug!(
            "Sent profile header size: {} bytes",
            u32::from_le_bytes(size_bytes)
        );
    } else {
        // Fallback to default D0 size
        let header_size: u32 = crate::protocol::constants::D0_FW_UPDATE_PROFILE_HDR_SIZE as u32;
        ctx.transport.write(&header_size.to_le_bytes())?;
    }

    Ok(HandleResult::Continue)
}

/// RUPH - Ready for Update Profile Header.
fn handle_ruph<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("RUPH: Sending FW Update Profile Header");
    ctx.log(LogLevel::Debug, "Sending FW Update Profile Header");

    if let Some(fw) = ctx.fw_image {
        let header = fw.profile_header_bytes();
        ctx.transport.write(header)?;
        debug!("Sent profile header: {} bytes", header.len());
    } else {
        warn!("No FW image available for RUPH");
        ctx.log(LogLevel::Warn, "No FW image available");
    }

    Ok(HandleResult::Continue)
}

/// DMIP - Download MIP.
fn handle_dmip<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("DMIP: Sending MIP (Module Info Pointer)");
    ctx.log(LogLevel::Debug, "Sending MIP");

    // MIP is typically embedded in the DnX header region
    // For now, we acknowledge but the actual MIP extraction may need refinement
    if let Some(fw) = ctx.fw_image {
        let dnx_header = fw.dnx_header_bytes();
        ctx.transport.write(dnx_header)?;
        debug!("Sent DnX header as MIP: {} bytes", dnx_header.len());
    }

    Ok(HandleResult::Continue)
}

/// LOFW - Low FW (first 128KB).
fn handle_lofw<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("LOFW: Sending first 128KB FW chunk");
    ctx.log(LogLevel::Debug, "Sending first 128KB chunk");

    if let Some(fw) = ctx.fw_image {
        let lofw = fw.lofw_bytes();
        if !lofw.is_empty() {
            ctx.transport.write(lofw)?;
            ctx.emit(DnxEvent::Progress {
                phase: crate::events::DnxPhase::FirmwareDownload,
                operation: "LOFW".to_string(),
                current: lofw.len() as u64,
                total: lofw.len() as u64,
            });
            debug!("Sent LOFW: {} bytes", lofw.len());
        } else {
            warn!("LOFW data is empty");
        }
    } else {
        warn!("No FW image available for LOFW");
    }

    Ok(HandleResult::Continue)
}

/// HIFW - High FW (second 128KB).
fn handle_hifw<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("HIFW: Sending second 128KB FW chunk");
    ctx.log(LogLevel::Debug, "Sending second 128KB chunk");

    if let Some(fw) = ctx.fw_image {
        let hifw = fw.hifw_bytes();
        if !hifw.is_empty() {
            ctx.transport.write(hifw)?;
            ctx.emit(DnxEvent::Progress {
                phase: crate::events::DnxPhase::FirmwareDownload,
                operation: "HIFW".to_string(),
                current: hifw.len() as u64,
                total: hifw.len() as u64,
            });
            debug!("Sent HIFW: {} bytes", hifw.len());
        } else {
            warn!("HIFW data is empty");
        }
    } else {
        warn!("No FW image available for HIFW");
    }

    Ok(HandleResult::Continue)
}

/// PSFW1 - Primary Security FW 1.
fn handle_psfw1<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    debug!("PSFW1: Sending Primary Security FW 1 chunk");

    if let Some(fw) = ctx.fw_image {
        let psfw1 = fw.psfw1_bytes();
        if !psfw1.is_empty() {
            // Get next chunk using state
            if let Some(chunk) = ctx.state.psfw1_state.next_chunk(psfw1) {
                ctx.transport.write(chunk)?;
                ctx.emit(DnxEvent::Progress {
                    phase: crate::events::DnxPhase::FirmwareDownload,
                    operation: "PSFW1".to_string(),
                    current: ctx.state.psfw1_state.current as u64,
                    total: ctx.state.psfw1_state.total as u64,
                });
                debug!(
                    "PSFW1 chunk {}/{}: {} bytes",
                    ctx.state.psfw1_state.current,
                    ctx.state.psfw1_state.total,
                    chunk.len()
                );
            }
        }
    }

    Ok(HandleResult::Continue)
}

/// PSFW2 - Primary Security FW 2.
fn handle_psfw2<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    debug!("PSFW2: Sending Primary Security FW 2 chunk");

    if let Some(fw) = ctx.fw_image {
        let psfw2 = fw.psfw2_bytes();
        if !psfw2.is_empty() {
            if let Some(chunk) = ctx.state.psfw2_state.next_chunk(psfw2) {
                ctx.transport.write(chunk)?;
                ctx.emit(DnxEvent::Progress {
                    phase: crate::events::DnxPhase::FirmwareDownload,
                    operation: "PSFW2".to_string(),
                    current: ctx.state.psfw2_state.current as u64,
                    total: ctx.state.psfw2_state.total as u64,
                });
                debug!(
                    "PSFW2 chunk {}/{}: {} bytes",
                    ctx.state.psfw2_state.current,
                    ctx.state.psfw2_state.total,
                    chunk.len()
                );
            }
        }
    }

    Ok(HandleResult::Continue)
}

/// SSFW - Secondary Security FW.
fn handle_ssfw<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    debug!("SSFW: Sending Secondary Security FW chunk");

    if let Some(fw) = ctx.fw_image {
        let ssfw = fw.ssfw_bytes();
        if !ssfw.is_empty() {
            if let Some(chunk) = ctx.state.ssfw_state.next_chunk(ssfw) {
                ctx.transport.write(chunk)?;
                ctx.emit(DnxEvent::Progress {
                    phase: crate::events::DnxPhase::FirmwareDownload,
                    operation: "SSFW".to_string(),
                    current: ctx.state.ssfw_state.current as u64,
                    total: ctx.state.ssfw_state.total as u64,
                });
                debug!(
                    "SSFW chunk {}/{}: {} bytes",
                    ctx.state.ssfw_state.current,
                    ctx.state.ssfw_state.total,
                    chunk.len()
                );
            }
        }
    }

    Ok(HandleResult::Continue)
}

/// VEDFW - Video Encoder/Decoder FW.
fn handle_vedfw<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    debug!("VEDFW: Sending Video Encoder/Decoder FW chunk");

    if let Some(fw) = ctx.fw_image {
        let vedfw = fw.vedfw_bytes();
        if !vedfw.is_empty() {
            if let Some(chunk) = ctx.state.vedfw_state.next_chunk(vedfw) {
                ctx.transport.write(chunk)?;
                ctx.emit(DnxEvent::Progress {
                    phase: crate::events::DnxPhase::FirmwareDownload,
                    operation: "VEDFW".to_string(),
                    current: ctx.state.vedfw_state.current as u64,
                    total: ctx.state.vedfw_state.total as u64,
                });
                debug!(
                    "VEDFW chunk {}/{}: {} bytes",
                    ctx.state.vedfw_state.current,
                    ctx.state.vedfw_state.total,
                    chunk.len()
                );
            }
        }
    }

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

    if let Some(os) = ctx.os_image {
        let osip = os.osip_bytes();
        ctx.transport.write(osip)?;
        debug!("Sent OSIP: {} bytes", osip.len());

        // Initialize OS image chunk state for subsequent RIMG requests
        let image_data = os.image_data();
        ctx.state.os_image_state = crate::payload::OsChunkState::new(
            image_data.len(),
            crate::protocol::constants::ONE28_K,
        );
    } else {
        warn!("No OS image available for ROSIP");
    }

    Ok(HandleResult::Continue)
}

/// RIMG - Request Image chunk.
fn handle_rimg<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    debug!("RIMG: Sending OS image chunk");

    if let Some(os) = ctx.os_image {
        let image_data = os.image_data();
        if let Some(chunk) = ctx.state.os_image_state.next_chunk(image_data) {
            ctx.transport.write(chunk)?;
            ctx.emit(DnxEvent::Progress {
                phase: crate::events::DnxPhase::OsDownload,
                operation: "OS Image".to_string(),
                current: ctx.state.os_image_state.current as u64,
                total: ctx.state.os_image_state.total as u64,
            });
            debug!(
                "OS chunk {}/{}: {} bytes",
                ctx.state.os_image_state.current,
                ctx.state.os_image_state.total,
                chunk.len()
            );
        }
    }

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

/// Helper to find Chaabi range in DnX binary.
/// Returns (start, end) offsets for the Token+FW section (NOT including CDPH).
fn find_chaabi_range(data: &[u8]) -> Option<(usize, usize)> {
    let ch00_magic = b"CH00";
    let cdph_magic = b"CDPH";
    let dtkn_magic = b"DTKN";

    let find = |needle: &[u8]| -> Option<usize> {
        data.windows(needle.len())
            .position(|window| window == needle)
    };

    let ch00_pos = find(ch00_magic)?;
    let cdph_pos = find(cdph_magic)?;

    // Token+FW start: CH00 - 0x80, or DTKN if found
    let mut start = ch00_pos.checked_sub(0x80)?;
    if let Some(dtkn_pos) = data[..ch00_pos]
        .windows(dtkn_magic.len())
        .position(|w| w == dtkn_magic)
    {
        start = dtkn_pos;
    }

    // Token+FW end: CDPH position (CDPH is separate)
    let end = cdph_pos;

    if start < end && end <= data.len() {
        Some((start, end))
    } else {
        None
    }
}

/// Build Chaabi payload with correct structure for device.
/// According to xFSTK's InitDnx(), the structure is:
/// [CDPH Header (24 bytes from FILE END)] + [Token + FW data]
///
/// Token markers (in order of priority):
/// - DTKN: TNG B0+ token container (16KB)
/// - $CHT: TNG A0 token (starts at $CHT - 0x80)
/// - ChPr: TNG B0/ANN token
/// - None: fallback to CH00 - 0x80
fn build_chaabi_payload(data: &[u8]) -> Option<Vec<u8>> {
    let ch00_magic = b"CH00";
    let cdph_magic = b"CDPH";
    let dtkn_magic = b"DTKN";
    let cht_magic = b"$CHT"; // TNG A0
    let chpr_magic = b"ChPr"; // TNG B0/ANN

    let find = |needle: &[u8]| -> Option<usize> {
        data.windows(needle.len())
            .position(|window| window == needle)
    };

    let ch00_pos = find(ch00_magic)?;
    let cdph_pos = find(cdph_magic)?;
    let file_size = data.len();

    // Determine Token+FW start position based on available markers
    // Priority: DTKN > $CHT > ChPr > CH00-0x80
    let token_fw_start = if let Some(dtkn_pos) = find(dtkn_magic) {
        // DTKN found - B0+ token container
        if dtkn_pos < ch00_pos {
            tracing::info!("Using DTKN marker at 0x{:x} for Token start", dtkn_pos);
            dtkn_pos
        } else {
            ch00_pos.checked_sub(0x80)?
        }
    } else if let Some(cht_pos) = find(cht_magic) {
        // $CHT found - TNG A0 token
        if cht_pos < ch00_pos {
            let start = cht_pos.checked_sub(0x80)?;
            tracing::info!(
                "Using $CHT marker at 0x{:x}, Token starts at 0x{:x}",
                cht_pos,
                start
            );
            start
        } else {
            ch00_pos.checked_sub(0x80)?
        }
    } else if let Some(chpr_pos) = find(chpr_magic) {
        // ChPr found - TNG B0/ANN token
        if chpr_pos < ch00_pos {
            tracing::info!("Using ChPr marker at 0x{:x} for Token start", chpr_pos);
            chpr_pos
        } else {
            ch00_pos.checked_sub(0x80)?
        }
    } else {
        // No token marker found, fallback to CH00 - 0x80
        tracing::info!("No token marker found, using CH00 - 0x80");
        ch00_pos.checked_sub(0x80)?
    };

    // Token+FW end: CDPH string position (not including CDPH itself)
    let token_fw_end = cdph_pos;

    // CDPH header: LAST 24 bytes of the FILE (not from CDPH string position!)
    if file_size < 24 {
        return None;
    }
    let cdph_header = &data[file_size - 24..file_size];
    let token_fw_data = &data[token_fw_start..token_fw_end];

    tracing::info!(
        "Chaabi payload: Header 24 bytes from file end, Body {} bytes from 0x{:x} to 0x{:x}",
        token_fw_data.len(),
        token_fw_start,
        token_fw_end
    );

    // Build: CDPH first (from file end), then Token+FW
    let mut payload = Vec::with_capacity(24 + token_fw_data.len());
    payload.extend_from_slice(cdph_header);
    payload.extend_from_slice(token_fw_data);

    Some(payload)
}
