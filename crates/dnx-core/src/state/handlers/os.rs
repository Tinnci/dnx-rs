//! OS download handlers (DORM, ROSIP, RIMG, EOIU).

use crate::events::{DnxEvent, DnxObserver, DnxPhase, LogLevel};
use crate::state::machine::DldrState;
use crate::transport::UsbTransport;
use anyhow::Result;
use tracing::{debug, info, warn};

use super::{HandleResult, HandlerContext};

/// DORM - OS Recovery Mode.
pub fn handle_dorm<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("DORM: Entering OS Recovery mode");
    ctx.log(LogLevel::Info, "Entering OS Recovery mode");
    ctx.state.goto_state(DldrState::OsNormal);
    Ok(HandleResult::Continue)
}

/// ROSIP - Ready for OSIP.
pub fn handle_rosip<T: UsbTransport, O: DnxObserver>(
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
pub fn handle_rimg<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    debug!("RIMG: Sending OS image chunk");

    if let Some(os) = ctx.os_image {
        let image_data = os.image_data();
        if let Some(chunk) = ctx.state.os_image_state.next_chunk(image_data) {
            ctx.transport.write(chunk)?;
            ctx.emit(DnxEvent::Progress {
                phase: DnxPhase::OsDownload,
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
pub fn handle_eoiu<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    info!("EOIU: OS image transfer complete");
    ctx.log(LogLevel::Info, "OS image transfer complete");
    Ok(HandleResult::Continue)
}
