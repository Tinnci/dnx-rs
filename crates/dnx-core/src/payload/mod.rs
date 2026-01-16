//! Payload handling module.
//!
//! Provides parsing and chunking for firmware and OS images.

pub mod firmware;
pub mod os;

pub use firmware::{ChunkIterator, ChunkState, FirmwareError, FirmwareImage, FwComponent};
pub use os::{OsChunkIterator, OsChunkState, OsImage, OsImageError};
