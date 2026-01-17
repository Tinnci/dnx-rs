//! Firmware download handlers (DFRM, DxxM, DCFI, DIFWI, DXBL, RUPH, DMIP, LOFW, HIFW).

use crate::events::{DnxEvent, DnxObserver, DnxPhase, LogLevel};
use crate::state::machine::DldrState;
use crate::transport::UsbTransport;
use anyhow::Result;
use tracing::{debug, info, warn};

use super::chaabi::{build_chaabi_payload, find_chaabi_range};
use super::{HandleResult, HandlerContext};

/// DFRM - Virgin part DnX.
pub fn handle_dfrm<T: UsbTransport, O: DnxObserver>(
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
        from: DnxPhase::Handshake,
        to: DnxPhase::FirmwareDownload,
    });
    ctx.state.goto_state(DldrState::FwNormal);
    Ok(HandleResult::Continue)
}

/// DxxM - Non-virgin part DnX.
pub fn handle_dxxm<T: UsbTransport, O: DnxObserver>(
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
        from: DnxPhase::Handshake,
        to: DnxPhase::FirmwareDownload,
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
pub fn handle_dcfi00<T: UsbTransport, O: DnxObserver>(
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
                phase: DnxPhase::FirmwareDownload,
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
pub fn handle_difwi<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    debug!("DIFWI: Device requested IFWI chunk");

    // We assume IFWI data corresponds to 0..chaabi_start in the dnx_fwr.bin
    // This state should have been initialized in handle_dcfi00 success path.
    // If not, we try to initialize it here or fail.

    if ctx.state.ifwi_state.total == 0 {
        // Not initialized? Try to find boundaries again.
        if let Some(dnx_data) = ctx.fw_dnx_data
            && let Some((start, _)) = find_chaabi_range(dnx_data)
        {
            let ifwi_len = start;
            ctx.state.ifwi_state =
                crate::payload::ChunkState::new(ifwi_len, crate::protocol::constants::ONE28_K);
        }
    }

    if let Some(dnx_data) = ctx.fw_dnx_data {
        // Efficient way: re-find range (it's fast)
        if let Some((chaabi_start, _)) = find_chaabi_range(dnx_data) {
            let ifwi_data = &dnx_data[0..chaabi_start];

            if let Some(chunk) = ctx.state.ifwi_state.next_chunk(ifwi_data) {
                ctx.transport.write(chunk)?;
                ctx.emit(DnxEvent::Progress {
                    phase: DnxPhase::FirmwareDownload,
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
pub fn handle_dxbl<T: UsbTransport, O: DnxObserver>(
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
            phase: DnxPhase::FirmwareDownload,
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
pub fn handle_ruphs<T: UsbTransport, O: DnxObserver>(
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
pub fn handle_ruph<T: UsbTransport, O: DnxObserver>(
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
pub fn handle_dmip<T: UsbTransport, O: DnxObserver>(
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
pub fn handle_lofw<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("LOFW: Sending first 128KB FW chunk");
    ctx.log(LogLevel::Debug, "Sending first 128KB chunk");

    if let Some(fw) = ctx.fw_image {
        let lofw = fw.lofw_bytes();
        if !lofw.is_empty() {
            ctx.transport.write(lofw)?;
            ctx.emit(DnxEvent::Progress {
                phase: DnxPhase::FirmwareDownload,
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
pub fn handle_hifw<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("HIFW: Sending second 128KB FW chunk");
    ctx.log(LogLevel::Debug, "Sending second 128KB chunk");

    if let Some(fw) = ctx.fw_image {
        let hifw = fw.hifw_bytes();
        if !hifw.is_empty() {
            ctx.transport.write(hifw)?;
            ctx.emit(DnxEvent::Progress {
                phase: DnxPhase::FirmwareDownload,
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
