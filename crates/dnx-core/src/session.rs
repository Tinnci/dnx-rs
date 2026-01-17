//! DnX Session - High-level orchestrator for the download process.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{Result, anyhow};
use tracing::{info, instrument, warn};

use crate::events::{DnxEvent, DnxObserver, DnxPhase, PacketDirection, TracingObserver};
use crate::protocol::constants::PREAMBLE_DNER;
use crate::state::handlers::{HandleResult, HandlerContext, handle_ack};
use crate::state::machine::StateMachineContext;
use crate::transport::{NusbTransport, TransportError, UsbTransport};
use serde::{Deserialize, Serialize};

/// Configuration for a DnX session.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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

impl SessionConfig {
    /// Load configuration from a TOML file
    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: SessionConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to a TOML file
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
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

        let mut state = StateMachineContext::new();
        state.gp_flags = self.config.gp_flags;
        state.ifwi_wipe_enable = self.config.ifwi_wipe_enable;

        loop {
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

            // Wrap transport with observer
            let obs_transport = ObservableTransport {
                inner: &transport,
                observer: &self.observer,
            };

            // Run state machine
            let result = self.run_state_machine(&obs_transport, &mut state);

            match result {
                Ok(HandleResult::Complete) => break,
                Ok(HandleResult::NeedReEnumerate) => {
                    info!("Device resetting, waiting for re-enumeration...");
                    thread::sleep(Duration::from_secs(2)); // Wait for device to actually disconnect
                    continue; // Loop back to wait_for_device
                }
                Ok(_) => break, // Other results end the session normally
                Err(e) => return Err(e),
            }
        }

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

    fn run_state_machine<T: UsbTransport>(
        &self,
        transport: &T,
        state: &mut StateMachineContext,
    ) -> Result<HandleResult> {
        // Send initial preamble only if we are starting fresh or after a reset that returns to DnX mode
        if !state.gpp_reset {
            self.observer.on_event(&DnxEvent::PhaseChanged {
                from: DnxPhase::WaitingForDevice,
                to: DnxPhase::Handshake,
            });

            // Initial handshake: send DnER.
            // Most devices (including Moorefield 0A2C/0A65) respond to this.
            transport.write(&PREAMBLE_DNER.to_le_bytes())?;
            info!(preamble = "DnER", "Sent handshake preamble");

            // We used to send IDRQ immediately for Moorefield here, but it caused
            // "hardware fault or protocol violation" (EPROTO) on some devices.
            // We'll now wait for the first response in the main loop instead.
        } else {
            // After reset, we might just wait for the first ACK from the new stage
            info!("Resuming state machine after reset");
            state.gpp_reset = false;
        }

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
                    return Ok(HandleResult::NeedReEnumerate);
                }
                Err(e) => {
                    // Intel xFSTK uses extensive retries.
                    // We shouldn't fail immediately on transient read errors.
                    // Log it as a debug/warn but keep trying.
                    warn!(error = ?e, "Transient read error, retrying...");
                    thread::sleep(Duration::from_millis(50));
                    continue;
                }
            };

            let mut ctx = HandlerContext {
                transport,
                observer: self.observer.as_ref(),
                state,
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
                    return Ok(HandleResult::Complete);
                }
                HandleResult::Error(msg) => {
                    return Err(anyhow!(msg));
                }
                HandleResult::NeedReEnumerate => {
                    self.observer.on_event(&DnxEvent::PhaseChanged {
                        from: DnxPhase::FirmwareDownload,
                        to: DnxPhase::DeviceReset,
                    });
                    self.observer.on_event(&DnxEvent::DeviceDisconnected);
                    return Ok(HandleResult::NeedReEnumerate);
                }
            }

            if !state.should_continue() {
                break;
            }
        }

        Ok(HandleResult::Complete)
    }
}

/// Transport wrapper that emits packet events.
struct ObservableTransport<'a, T: UsbTransport, O: DnxObserver> {
    inner: &'a T,
    observer: &'a Arc<O>,
}

impl<'a, T: UsbTransport, O: DnxObserver> UsbTransport for ObservableTransport<'a, T, O> {
    fn write(&self, data: &[u8]) -> Result<usize, TransportError> {
        let res = self.inner.write(data);
        if res.is_ok() {
            let packet_type = if data.len() < 32 { "Cmd/Hdr" } else { "Data" };
            self.observer.on_event(&DnxEvent::Packet {
                direction: PacketDirection::Tx,
                packet_type: packet_type.to_string(),
                length: data.len(),
                data: Some(data.iter().take(32).cloned().collect()),
            });
        }
        res
    }

    fn read(&self, max_len: usize) -> Result<Vec<u8>, TransportError> {
        let res = self.inner.read(max_len);
        if let Ok(data) = &res
            && !data.is_empty()
        {
            self.observer.on_event(&DnxEvent::Packet {
                direction: PacketDirection::Rx,
                packet_type: "Data".to_string(),
                length: data.len(),
                data: Some(data.iter().take(32).cloned().collect()),
            });
        }
        res
    }

    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    fn vendor_id(&self) -> u16 {
        self.inner.vendor_id()
    }

    fn product_id(&self) -> u16 {
        self.inner.product_id()
    }
}
