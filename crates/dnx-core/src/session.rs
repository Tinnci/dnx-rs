//! DnX Session - High-level orchestrator for the download process.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{Result, anyhow};
use tracing::{info, instrument, warn};

use crate::events::{DnxEvent, DnxObserver, DnxPhase, TracingObserver};
use crate::protocol::constants::PREAMBLE_DNER;
use crate::state::handlers::{HandleResult, HandlerContext, handle_ack};
use crate::state::machine::StateMachineContext;
use crate::transport::{NusbTransport, TransportError, UsbTransport};

/// Configuration for a DnX session.
#[derive(Debug, Default, Clone)]
pub struct SessionConfig {
    /// Path to FW DnX binary.
    pub fw_dnx_path: Option<String>,
    /// Path to FW image (IFWI).
    pub fw_image_path: Option<String>,
    /// Path to OS DnX binary.
    pub os_dnx_path: Option<String>,
    /// Path to OS image.
    pub os_image_path: Option<String>,
    /// Path to Misc DnX binary.
    pub misc_dnx_path: Option<String>,
    /// GP flags.
    pub gp_flags: u32,
    /// Enable IFWI wipe.
    pub ifwi_wipe_enable: bool,
    /// Retry timeout in seconds.
    pub retry_timeout_secs: u64,
}

/// DnX Session - orchestrates the complete download process.
pub struct DnxSession<O: DnxObserver> {
    config: SessionConfig,
    observer: Arc<O>,
    // Loaded file data
    fw_dnx_data: Option<Vec<u8>>,
    fw_image: Option<crate::payload::FirmwareImage>,
    os_dnx_data: Option<Vec<u8>>,
    os_image: Option<crate::payload::OsImage>,
}

impl DnxSession<TracingObserver> {
    /// Create a new session with default tracing observer.
    pub fn new(config: SessionConfig) -> Self {
        Self::with_observer(config, Arc::new(TracingObserver))
    }
}

impl<O: DnxObserver + 'static> DnxSession<O> {
    /// Create a new session with a custom observer.
    pub fn with_observer(config: SessionConfig, observer: Arc<O>) -> Self {
        Self {
            config,
            observer,
            fw_dnx_data: None,
            fw_image: None,
            os_dnx_data: None,
            os_image: None,
        }
    }

    /// Load all required files.
    fn load_files(&mut self) -> Result<()> {
        if let Some(path) = &self.config.fw_dnx_path {
            info!(path = %path, "Loading FW DnX");
            self.fw_dnx_data = Some(std::fs::read(path)?);
        }
        if let Some(path) = &self.config.fw_image_path {
            info!(path = %path, "Loading FW Image");
            let data = std::fs::read(path)?;
            self.fw_image = Some(crate::payload::FirmwareImage::from_bytes(data)?);
        }
        if let Some(path) = &self.config.os_dnx_path {
            info!(path = %path, "Loading OS DnX");
            self.os_dnx_data = Some(std::fs::read(path)?);
        }
        if let Some(path) = &self.config.os_image_path {
            info!(path = %path, "Loading OS Image");
            let data = std::fs::read(path)?;
            self.os_image = Some(crate::payload::OsImage::from_bytes(data)?);
        }
        Ok(())
    }

    /// Run the complete DnX session.
    #[instrument(skip(self))]
    pub fn run(&mut self) -> Result<()> {
        // Load files
        self.load_files()?;

        // Emit starting event
        self.observer.on_event(&DnxEvent::PhaseChanged {
            from: DnxPhase::WaitingForDevice,
            to: DnxPhase::WaitingForDevice,
        });

        // Wait for device
        let transport = self.wait_for_device()?;

        self.observer.on_event(&DnxEvent::DeviceConnected {
            vid: transport.vendor_id(),
            pid: transport.product_id(),
        });

        // Run state machine
        self.run_state_machine(&transport)?;

        Ok(())
    }

    fn wait_for_device(&self) -> Result<NusbTransport> {
        info!("Waiting for device...");
        let timeout = Duration::from_secs(self.config.retry_timeout_secs.max(60));
        let start = std::time::Instant::now();
        let mut poll_count = 0u64;

        loop {
            poll_count += 1;

            match NusbTransport::open() {
                Ok(t) => {
                    info!(
                        vid = format!("{:04X}", t.vendor_id()),
                        pid = format!("{:04X}", t.product_id()),
                        "Device found after {} polls",
                        poll_count
                    );
                    return Ok(t);
                }
                Err(TransportError::DeviceNotFound { .. }) => {
                    if start.elapsed() > timeout {
                        return Err(anyhow!(
                            "Timeout waiting for device after {}s",
                            timeout.as_secs()
                        ));
                    }
                    // Fast polling: 100ms instead of 1s
                    thread::sleep(Duration::from_millis(100));
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    fn run_state_machine(&self, transport: &NusbTransport) -> Result<()> {
        let mut state = StateMachineContext::new();
        state.gp_flags = self.config.gp_flags;
        state.ifwi_wipe_enable = self.config.ifwi_wipe_enable;

        // Send initial preamble
        self.observer.on_event(&DnxEvent::PhaseChanged {
            from: DnxPhase::WaitingForDevice,
            to: DnxPhase::Handshake,
        });

        transport.write(&PREAMBLE_DNER.to_le_bytes())?;
        info!(preamble = "DnER", "Sent preamble");

        // Main loop
        loop {
            let ack = match transport.read_ack() {
                Ok(a) => a,
                Err(TransportError::Timeout { .. }) => {
                    continue;
                }
                Err(TransportError::Disconnected) => {
                    self.observer.on_event(&DnxEvent::DeviceDisconnected);
                    warn!("Device disconnected");
                    // TODO: Handle re-enumeration
                    return Err(anyhow!("Device disconnected"));
                }
                Err(e) => {
                    warn!(error = ?e, "Read error");
                    thread::sleep(Duration::from_millis(100));
                    continue;
                }
            };

            let mut ctx = HandlerContext {
                transport,
                observer: self.observer.as_ref(),
                state: &mut state,
                fw_dnx_data: self.fw_dnx_data.as_deref(),
                fw_image: self.fw_image.as_ref(),
                os_dnx_data: self.os_dnx_data.as_deref(),
                os_image: self.os_image.as_ref(),
            };

            let result = handle_ack(&ack, &mut ctx)?;

            match result {
                HandleResult::Continue => {}
                HandleResult::FwDone => {
                    self.observer.on_event(&DnxEvent::PhaseChanged {
                        from: DnxPhase::FirmwareDownload,
                        to: DnxPhase::OsDownload,
                    });
                }
                HandleResult::OsDone => {
                    self.observer.on_event(&DnxEvent::PhaseChanged {
                        from: DnxPhase::OsDownload,
                        to: DnxPhase::Complete,
                    });
                }
                HandleResult::Complete => {
                    self.observer.on_event(&DnxEvent::Complete);
                    return Ok(());
                }
                HandleResult::Error(msg) => {
                    return Err(anyhow!(msg));
                }
                HandleResult::NeedReEnumerate => {
                    self.observer.on_event(&DnxEvent::PhaseChanged {
                        from: DnxPhase::FirmwareDownload,
                        to: DnxPhase::DeviceReset,
                    });
                    // TODO: Implement re-enumeration logic
                    self.observer.on_event(&DnxEvent::DeviceDisconnected);
                    return Err(anyhow!("Device reset - re-enumeration not implemented"));
                }
            }

            if !state.should_continue() {
                break;
            }
        }

        Ok(())
    }
}
