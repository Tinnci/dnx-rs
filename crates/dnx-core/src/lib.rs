//! DnX-Core: Intel DnX protocol implementation in Rust.
//!
//! This crate provides a complete implementation of the Intel Medfield/Merrifield
//! DnX (Download and Execute) protocol for firmware and OS recovery.
//!
//! # Architecture
//!
//! The crate is organized into layers:
//!
//! - **Protocol**: Constants, ACK codes, header structures
//! - **Transport**: USB communication abstraction (nusb, mock)
//! - **State**: State machine and ACK handlers
//! - **Events**: Observer pattern for UI decoupling
//! - **Session**: High-level orchestrator
//!
//! # Example
//!
//! ```no_run
//! use dnx_core::session::{DnxSession, SessionConfig};
//!
//! let config = SessionConfig {
//!     fw_dnx_path: Some("dnx_fwr.bin".to_string()),
//!     fw_image_path: Some("ifwi.bin".to_string()),
//!     ..Default::default()
//! };
//!
//! let mut session = DnxSession::new(config);
//! session.run().expect("DnX failed");
//! ```

pub mod events;
pub mod protocol;
pub mod session;
pub mod state;
pub mod transport;

// Re-exports for convenience
pub use events::{DnxEvent, DnxObserver, DnxPhase, LogLevel, TracingObserver};
pub use protocol::AckCode;
pub use session::{DnxSession, SessionConfig};
pub use transport::{MockTransport, NusbTransport, TransportError, UsbTransport};
