//! Security firmware handlers (PSFW, SSFW, VEDFW).

use crate::events::{DnxEvent, DnxObserver, DnxPhase};
use crate::transport::UsbTransport;
use anyhow::Result;
use tracing::debug;

use super::{HandleResult, HandlerContext};

/// PSFW1 - Primary Security FW 1.
pub fn handle_psfw1<T: UsbTransport, O: DnxObserver>(
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
                    phase: DnxPhase::FirmwareDownload,
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
pub fn handle_psfw2<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    debug!("PSFW2: Sending Primary Security FW 2 chunk");

    if let Some(fw) = ctx.fw_image {
        let psfw2 = fw.psfw2_bytes();
        if !psfw2.is_empty()
            && let Some(chunk) = ctx.state.psfw2_state.next_chunk(psfw2)
        {
            ctx.transport.write(chunk)?;
            ctx.emit(DnxEvent::Progress {
                phase: DnxPhase::FirmwareDownload,
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

    Ok(HandleResult::Continue)
}

/// SSFW - Secondary Security FW.
pub fn handle_ssfw<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    debug!("SSFW: Sending Secondary Security FW chunk");

    if let Some(fw) = ctx.fw_image {
        let ssfw = fw.ssfw_bytes();
        if !ssfw.is_empty()
            && let Some(chunk) = ctx.state.ssfw_state.next_chunk(ssfw)
        {
            ctx.transport.write(chunk)?;
            ctx.emit(DnxEvent::Progress {
                phase: DnxPhase::FirmwareDownload,
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

    Ok(HandleResult::Continue)
}

/// VEDFW - Video Encoder/Decoder FW.
pub fn handle_vedfw<T: UsbTransport, O: DnxObserver>(
    ctx: &mut HandlerContext<'_, T, O>,
) -> Result<HandleResult> {
    debug!("VEDFW: Sending Video Encoder/Decoder FW chunk");

    if let Some(fw) = ctx.fw_image {
        let vedfw = fw.vedfw_bytes();
        if !vedfw.is_empty()
            && let Some(chunk) = ctx.state.vedfw_state.next_chunk(vedfw)
        {
            ctx.transport.write(chunk)?;
            ctx.emit(DnxEvent::Progress {
                phase: DnxPhase::FirmwareDownload,
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

    Ok(HandleResult::Continue)
}
