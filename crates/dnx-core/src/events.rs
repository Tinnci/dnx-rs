//! Event system for UI decoupling.
//!
//! Allows CLI/TUI/GUI to subscribe to protocol events without
//! tight coupling to the core logic.

use std::fmt;

/// Log level for events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// DnX state machine phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnxPhase {
    /// Waiting for device connection.
    WaitingForDevice,
    /// Initial handshake (sending preamble).
    Handshake,
    /// Firmware download in progress.
    FirmwareDownload,
    /// OS download in progress.
    OsDownload,
    /// Device is resetting (GPP Reset).
    DeviceReset,
    /// All operations complete.
    Complete,
    /// Error state.
    Error,
}

impl fmt::Display for DnxPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DnxPhase::WaitingForDevice => write!(f, "Waiting for Device"),
            DnxPhase::Handshake => write!(f, "Handshake"),
            DnxPhase::FirmwareDownload => write!(f, "Firmware Download"),
            DnxPhase::OsDownload => write!(f, "OS Download"),
            DnxPhase::DeviceReset => write!(f, "Device Reset"),
            DnxPhase::Complete => write!(f, "Complete"),
            DnxPhase::Error => write!(f, "Error"),
        }
    }
}

/// Events emitted by the DnX session.
#[derive(Debug, Clone)]
pub enum DnxEvent {
    /// Device connected.
    DeviceConnected { vid: u16, pid: u16 },
    /// Device disconnected (might re-enumerate with different PID).
    DeviceDisconnected,
    /// Phase changed.
    PhaseChanged { from: DnxPhase, to: DnxPhase },
    /// Progress update for current operation.
    Progress {
        phase: DnxPhase,
        operation: String,
        current: u64,
        total: u64,
    },
    /// Log message.
    Log { level: LogLevel, message: String },
    /// ACK received from device.
    AckReceived { ack: String },
    /// Error occurred.
    Error { code: u32, message: String },
    /// USB Packet sent/received.
    Packet {
        direction: PacketDirection,
        packet_type: String,
        length: usize,
        data: Option<Vec<u8>>,
    },
    /// All operations completed successfully.
    Complete,
}

/// USB packet direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketDirection {
    Tx, // Transmit (Host -> Device)
    Rx, // Receive (Device -> Host)
}

impl fmt::Display for PacketDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PacketDirection::Tx => write!(f, "TX"),
            PacketDirection::Rx => write!(f, "RX"),
        }
    }
}

/// Observer trait for receiving DnX events.
///
/// Implement this trait in your UI layer to receive updates.
pub trait DnxObserver: Send + Sync {
    /// Called when an event occurs.
    fn on_event(&self, event: &DnxEvent);
}

/// No-op observer that discards all events.
pub struct NullObserver;

impl DnxObserver for NullObserver {
    fn on_event(&self, _event: &DnxEvent) {
        // Do nothing
    }
}

/// Observer that logs events using tracing.
pub struct TracingObserver;

impl DnxObserver for TracingObserver {
    fn on_event(&self, event: &DnxEvent) {
        match event {
            DnxEvent::DeviceConnected { vid, pid } => {
                tracing::info!(vid = %format!("{:04X}", vid), pid = %format!("{:04X}", pid), "Device connected");
            }
            DnxEvent::DeviceDisconnected => {
                tracing::warn!("Device disconnected");
            }
            DnxEvent::PhaseChanged { from, to } => {
                tracing::info!(from = %from, to = %to, "Phase changed");
            }
            DnxEvent::Progress {
                phase,
                operation,
                current,
                total,
            } => {
                let pct = if *total > 0 {
                    (*current * 100) / *total
                } else {
                    0
                };
                tracing::debug!(phase = %phase, operation = %operation, progress = %format!("{}%", pct), "Progress");
            }
            DnxEvent::Log { level, message } => match level {
                LogLevel::Trace => tracing::trace!("{}", message),
                LogLevel::Debug => tracing::debug!("{}", message),
                LogLevel::Info => tracing::info!("{}", message),
                LogLevel::Warn => tracing::warn!("{}", message),
                LogLevel::Error => tracing::error!("{}", message),
            },
            DnxEvent::AckReceived { ack } => {
                tracing::debug!(ack = %ack, "ACK received");
            }
            DnxEvent::Error { code, message } => {
                tracing::error!(code = code, "Error: {}", message);
            }
            DnxEvent::Packet {
                direction,
                packet_type,
                length,
                ..
            } => {
                tracing::trace!(
                    dir = %direction,
                    type_ = %packet_type,
                    len = length,
                    "USB Packet"
                );
            }
            DnxEvent::Complete => {
                tracing::info!("Operation complete");
            }
        }
    }
}
