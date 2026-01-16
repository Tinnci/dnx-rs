//! DnX state machine logic.

use crate::device::DnxDevice;
use crate::protocol::*;
use anyhow::Result;
use std::fs;
use std::thread;
use std::time::Duration;
use tracing::{debug, info, instrument, warn};

#[derive(Debug)]
pub struct DnxContext {
    pub fw_dnx_path: Option<String>,
    pub fw_image_path: Option<String>,
    pub os_dnx_path: Option<String>,
    pub os_image_path: Option<String>,
    pub misc_dnx_path: Option<String>,
}

/// Run the DnX protocol state machine.
/// This is a blocking function that handles device communication.
#[instrument(skip(ctx))]
pub fn run_state_machine(ctx: &DnxContext) -> Result<()> {
    info!("Waiting for device...");

    // Simple retry loop to find device
    let device = loop {
        match DnxDevice::open() {
            Ok(d) => break d,
            Err(_) => {
                thread::sleep(Duration::from_secs(1));
                continue;
            }
        }
    };

    info!("Device connected.");

    // Send initial preamble DnER
    let preamble: u32 = PREAMBLE_DNER;
    device.write(&preamble.to_le_bytes())?;
    info!(preamble = "DnER", "Sent preamble");

    loop {
        let ack_bytes = match device.read_ack() {
            Ok(b) => b,
            Err(e) => {
                warn!(error = ?e, "Read error (maybe re-enumeration?)");
                thread::sleep(Duration::from_millis(100));
                continue;
            }
        };

        if ack_bytes.is_empty() {
            continue;
        }

        // Parse ACK
        let ack_str = String::from_utf8_lossy(&ack_bytes);
        debug!(ack_bytes = ?ack_bytes, ack_str = %ack_str, "Received ACK");

        if check_ack(&ack_bytes, BULK_ACK_DFRM) {
            info!("Received DFRM (Virgin Part DnX)");
            if let Some(path) = &ctx.fw_dnx_path {
                send_dnx(&device, path)?;
            }
        } else if check_ack(&ack_bytes, BULK_ACK_DXBL) {
            info!("Received DXBL (Download Execute Bootloader)");
        } else if check_ack(&ack_bytes, BULK_ACK_READY_UPH) {
            info!("Received RUPH");
        } else if check_ack_u64(&ack_bytes, BULK_ACK_READY_UPH_SIZE) {
            info!("Received RUPHS");
        } else if check_ack(&ack_bytes, BULK_ACK_DONE) {
            info!("Received DONE");
            break;
        } else {
            warn!(ack = ?ack_bytes, "Unhandled ACK");
        }
    }

    Ok(())
}

fn check_ack(bytes: &[u8], expected: u32) -> bool {
    if bytes.len() < 4 {
        return false;
    }
    bytes.starts_with(&expected.to_be_bytes()) || bytes.starts_with(&expected.to_le_bytes())
}

fn check_ack_u64(bytes: &[u8], expected: u64) -> bool {
    if bytes.len() < 5 {
        return false;
    }
    let be = expected.to_be_bytes();
    // Skip leading zeros
    let start = be.iter().position(|&b| b != 0).unwrap_or(8);
    let meaningful = &be[start..];
    if meaningful.is_empty() {
        return false;
    }
    bytes.starts_with(meaningful)
}

fn send_dnx(device: &DnxDevice, path: &str) -> Result<()> {
    info!(path = %path, "Sending DnX binary");
    let data = fs::read(path)?;
    device.write(&data)?;
    Ok(())
}
