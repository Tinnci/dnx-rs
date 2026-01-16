//! USB Transport layer abstraction.
//!
//! Defines the `UsbTransport` trait for USB communication,
//! allowing different implementations (nusb, mock, etc.).

use crate::protocol::AckCode;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("Device not found: VID={vid:04X} PID={pid:04X}")]
    DeviceNotFound { vid: u16, pid: u16 },

    #[error("Failed to open device: {0}")]
    OpenFailed(String),

    #[error("Failed to claim interface {interface}: {message}")]
    ClaimInterfaceFailed { interface: u8, message: String },

    #[error("Endpoint not found: type={ep_type}, direction={direction}")]
    EndpointNotFound { ep_type: String, direction: String },

    #[error("Write failed: {0}")]
    WriteFailed(String),

    #[error("Read failed: {0}")]
    ReadFailed(String),

    #[error("Device disconnected")]
    Disconnected,

    #[error("Timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Abstract USB transport interface.
///
/// This trait enables:
/// - Production implementation using nusb
/// - Mock implementation for unit testing
/// - Future alternative backends
pub trait UsbTransport: Send + Sync {
    /// Write raw bytes to the OUT endpoint.
    fn write(&self, data: &[u8]) -> Result<usize, TransportError>;

    /// Read raw bytes from the IN endpoint.
    fn read(&self, max_len: usize) -> Result<Vec<u8>, TransportError>;

    /// Read and parse ACK code from device.
    fn read_ack(&self) -> Result<AckCode, TransportError> {
        let bytes = self.read(512)?;
        if bytes.is_empty() {
            return Err(TransportError::ReadFailed("Empty response".into()));
        }
        Ok(AckCode::from_bytes(&bytes))
    }

    /// Check if device is still connected.
    fn is_connected(&self) -> bool;

    /// Get the current VID.
    fn vendor_id(&self) -> u16;

    /// Get the current PID.
    fn product_id(&self) -> u16;
}
