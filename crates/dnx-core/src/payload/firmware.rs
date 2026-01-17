//! Firmware image parsing and chunking.
//!
//! Handles Intel Medfield firmware images including:
//! - DnX header parsing
//! - FW Update Profile Header extraction
//! - 128KB chunk iteration for PSFW1/PSFW2/SSFW/VEDFW
//!
//! Reference: xFSTK `dldrstate.cpp` StartFw(), FwHandlePSFW1, etc.

use crate::protocol::constants::ONE28_K;
use crate::protocol::header::{DnxHeader, FwUpdateProfileHeader, HeaderError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FirmwareError {
    #[error("File too small: {actual} bytes, minimum {minimum}")]
    FileTooSmall { actual: usize, minimum: usize },
    #[error("Invalid DnX magic")]
    InvalidMagic,
    #[error("Checksum mismatch: expected 0x{expected:08X}, got 0x{actual:08X}")]
    ChecksumMismatch { expected: u32, actual: u32 },
    #[error("Header error: {0}")]
    Header(#[from] HeaderError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Component not found: {0}")]
    ComponentNotFound(String),
}

/// Firmware component types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FwComponent {
    /// DnX header (first 24 bytes sent on RUPHS)
    DnxHeader,
    /// Profile header size (4 bytes)
    ProfileHeaderSize,
    /// Full profile header
    ProfileHeader,
    /// MIP (Module Info Pointer)
    Mip,
    /// Low FW (first 128KB)
    Lofw,
    /// High FW (second 128KB)  
    Hifw,
    /// Primary Security FW 1 (iCache)
    Psfw1,
    /// Primary Security FW 2 (Resident)
    Psfw2,
    /// Secondary Security FW (Extended)
    Ssfw,
    /// ROM Patch
    RomPatch,
    /// Video Encoder/Decoder FW
    VedFw,
}

/// Parsed firmware image with lazy component access.
#[derive(Debug)]
pub struct FirmwareImage {
    /// Raw firmware data
    data: Vec<u8>,
    /// Detected profile header size
    profile_header_size: usize,
    /// Offsets for various components (lazy parsed)
    psfw1_offset: usize,
    psfw1_size: usize,
    psfw2_offset: usize,
    psfw2_size: usize,
    ssfw_offset: usize,
    ssfw_size: usize,
    rom_patch_offset: usize,
    rom_patch_size: usize,
    vedfw_offset: usize,
    vedfw_size: usize,
}

impl FirmwareImage {
    /// Parse firmware image from raw bytes.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, FirmwareError> {
        // Minimum size: DnX header + some data
        if data.len() < DnxHeader::SIZE + 256 {
            return Err(FirmwareError::FileTooSmall {
                actual: data.len(),
                minimum: DnxHeader::SIZE + 256,
            });
        }

        // Detect profile header size by checking signature patterns
        // D0: 0x24, C0: 0x20, Old MFD: 0x1C
        let profile_header_size = Self::detect_profile_header_size(&data);

        // Parse profile header to get component sizes
        let header_start = DnxHeader::SIZE;
        let profile =
            FwUpdateProfileHeader::from_firmware_image(&data[header_start..], profile_header_size)?;

        let psfw1_size = profile.psfw1_size().unwrap_or(0) as usize;
        let psfw2_size = profile.psfw2_size().unwrap_or(0) as usize;
        let ssfw_size = profile.ssfw_size().unwrap_or(0) as usize;
        let rom_patch_size = profile.rom_patch_size().unwrap_or(0) as usize;

        // Calculate offsets
        // Layout: DnxHeader | ProfileHeader | LOFW (128K) | HIFW (128K) | PSFW1 | PSFW2 | SSFW | RomPatch | VEDFW
        let base = header_start + profile_header_size;
        let lofw_hifw_size = ONE28_K * 2; // 256KB for LOFW + HIFW

        let psfw1_offset = base + lofw_hifw_size;
        let psfw2_offset = psfw1_offset + psfw1_size;
        let ssfw_offset = psfw2_offset + psfw2_size;
        let rom_patch_offset = ssfw_offset + ssfw_size;
        let vedfw_offset = rom_patch_offset + rom_patch_size;
        let vedfw_size = data.len().saturating_sub(vedfw_offset);

        Ok(Self {
            data,
            profile_header_size,
            psfw1_offset,
            psfw1_size,
            psfw2_offset,
            psfw2_size,
            ssfw_offset,
            ssfw_size,
            rom_patch_offset,
            rom_patch_size,
            vedfw_offset,
            vedfw_size,
        })
    }

    fn detect_profile_header_size(_data: &[u8]) -> usize {
        // Try to detect based on known patterns
        // For now, default to D0 size
        FwUpdateProfileHeader::D0_SIZE
    }

    /// Get DnX header bytes.
    pub fn dnx_header_bytes(&self) -> &[u8] {
        &self.data[..DnxHeader::SIZE]
    }

    /// Get profile header size as u32 for sending.
    pub fn profile_header_size_bytes(&self) -> [u8; 4] {
        (self.profile_header_size as u32).to_le_bytes()
    }

    /// Get profile header bytes.
    pub fn profile_header_bytes(&self) -> &[u8] {
        let start = DnxHeader::SIZE;
        &self.data[start..start + self.profile_header_size]
    }

    /// Get LOFW (first 128KB after profile header).
    pub fn lofw_bytes(&self) -> &[u8] {
        let start = DnxHeader::SIZE + self.profile_header_size;
        let end = (start + ONE28_K).min(self.data.len());
        &self.data[start..end]
    }

    /// Get HIFW (second 128KB).
    pub fn hifw_bytes(&self) -> &[u8] {
        let start = DnxHeader::SIZE + self.profile_header_size + ONE28_K;
        let end = (start + ONE28_K).min(self.data.len());
        if start >= self.data.len() {
            return &[];
        }
        &self.data[start..end]
    }

    /// Get a chunk iterator for a specific component.
    pub fn chunk_iter(&self, component: FwComponent) -> ChunkIterator<'_> {
        let (data, chunk_size) = match component {
            FwComponent::Psfw1 => (self.psfw1_bytes(), ONE28_K),
            FwComponent::Psfw2 => (self.psfw2_bytes(), ONE28_K),
            FwComponent::Ssfw => (self.ssfw_bytes(), ONE28_K),
            FwComponent::RomPatch => (self.rom_patch_bytes(), ONE28_K),
            FwComponent::VedFw => (self.vedfw_bytes(), ONE28_K),
            _ => (&[] as &[u8], ONE28_K),
        };
        ChunkIterator::new(data, chunk_size)
    }

    /// Get PSFW1 bytes.
    pub fn psfw1_bytes(&self) -> &[u8] {
        if self.psfw1_offset >= self.data.len() || self.psfw1_size == 0 {
            return &[];
        }
        let end = (self.psfw1_offset + self.psfw1_size).min(self.data.len());
        &self.data[self.psfw1_offset..end]
    }

    /// Get PSFW2 bytes.
    pub fn psfw2_bytes(&self) -> &[u8] {
        if self.psfw2_offset >= self.data.len() || self.psfw2_size == 0 {
            return &[];
        }
        let end = (self.psfw2_offset + self.psfw2_size).min(self.data.len());
        &self.data[self.psfw2_offset..end]
    }

    /// Get SSFW bytes.
    pub fn ssfw_bytes(&self) -> &[u8] {
        if self.ssfw_offset >= self.data.len() || self.ssfw_size == 0 {
            return &[];
        }
        let end = (self.ssfw_offset + self.ssfw_size).min(self.data.len());
        &self.data[self.ssfw_offset..end]
    }

    /// Get ROM Patch bytes.
    pub fn rom_patch_bytes(&self) -> &[u8] {
        if self.rom_patch_offset >= self.data.len() || self.rom_patch_size == 0 {
            return &[];
        }
        let end = (self.rom_patch_offset + self.rom_patch_size).min(self.data.len());
        &self.data[self.rom_patch_offset..end]
    }

    /// Get VEDFW bytes.
    pub fn vedfw_bytes(&self) -> &[u8] {
        if self.vedfw_offset >= self.data.len() || self.vedfw_size == 0 {
            return &[];
        }
        let end = (self.vedfw_offset + self.vedfw_size).min(self.data.len());
        &self.data[self.vedfw_offset..end]
    }

    /// Get raw data.
    pub fn raw_data(&self) -> &[u8] {
        &self.data
    }

    /// Get total size.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Iterator over 128KB chunks with residual handling.
#[derive(Debug)]
pub struct ChunkIterator<'a> {
    data: &'a [u8],
    chunk_size: usize,
    offset: usize,
    total_chunks: usize,
    current_chunk: usize,
    residual_bytes: usize,
}

impl<'a> ChunkIterator<'a> {
    pub fn new(data: &'a [u8], chunk_size: usize) -> Self {
        let total_chunks = data.len() / chunk_size;
        let residual_bytes = data.len() % chunk_size;

        // If perfectly aligned and non-empty, we have total_chunks full chunks
        // If there's residual, we have total_chunks full + 1 partial
        Self {
            data,
            chunk_size,
            offset: 0,
            total_chunks,
            current_chunk: 0,
            residual_bytes,
        }
    }

    /// Get total number of chunks (including partial).
    pub fn total(&self) -> usize {
        if self.residual_bytes > 0 {
            self.total_chunks + 1
        } else {
            self.total_chunks
        }
    }

    /// Get current chunk index (0-based).
    pub fn current(&self) -> usize {
        self.current_chunk
    }

    /// Check if this is the last chunk.
    pub fn is_last(&self) -> bool {
        self.current_chunk + 1 >= self.total()
    }

    /// Reset iterator.
    pub fn reset(&mut self) {
        self.offset = 0;
        self.current_chunk = 0;
    }
}

impl<'a> Iterator for ChunkIterator<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.data.len() {
            return None;
        }

        let remaining = self.data.len() - self.offset;
        let chunk_len = remaining.min(self.chunk_size);

        let chunk = &self.data[self.offset..self.offset + chunk_len];
        self.offset += chunk_len;
        self.current_chunk += 1;

        Some(chunk)
    }
}

/// Chunk tracking state for stateful sending.
#[derive(Debug, Default, Clone)]
pub struct ChunkState {
    /// Current chunk index.
    pub current: usize,
    /// Total number of chunks.
    pub total: usize,
    /// Current byte offset.
    pub offset: usize,
    /// Size of each chunk.
    pub chunk_size: usize,
    /// Total data size.
    pub data_size: usize,
}

impl ChunkState {
    pub fn new(data_size: usize, chunk_size: usize) -> Self {
        let total = if data_size == 0 {
            0
        } else {
            data_size.div_ceil(chunk_size)
        };
        Self {
            current: 0,
            total,
            offset: 0,
            chunk_size,
            data_size,
        }
    }

    /// Get next chunk from data, advancing state.
    pub fn next_chunk<'a>(&mut self, data: &'a [u8]) -> Option<&'a [u8]> {
        if self.offset >= data.len() || self.offset >= self.data_size {
            return None;
        }

        let remaining = (self.data_size - self.offset).min(data.len() - self.offset);
        let chunk_len = remaining.min(self.chunk_size);

        let chunk = &data[self.offset..self.offset + chunk_len];
        self.offset += chunk_len;
        self.current += 1;

        Some(chunk)
    }

    /// Check if done.
    pub fn is_done(&self) -> bool {
        self.current >= self.total
    }

    /// Reset state.
    pub fn reset(&mut self) {
        self.current = 0;
        self.offset = 0;
    }

    /// Progress as percentage.
    pub fn progress_pct(&self) -> u8 {
        if self.total == 0 {
            100
        } else {
            ((self.current * 100) / self.total) as u8
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_iterator() {
        let data = vec![0u8; 300 * 1024]; // 300KB
        let iter = ChunkIterator::new(&data, ONE28_K);

        assert_eq!(iter.total(), 3); // 2 full + 1 partial (44KB)

        let chunks: Vec<_> = ChunkIterator::new(&data, ONE28_K).collect();
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), ONE28_K);
        assert_eq!(chunks[1].len(), ONE28_K);
        assert_eq!(chunks[2].len(), 300 * 1024 - 2 * ONE28_K);
    }

    #[test]
    fn test_chunk_state() {
        let data = vec![1u8; 300 * 1024];
        let mut state = ChunkState::new(data.len(), ONE28_K);

        assert_eq!(state.total, 3);
        assert!(!state.is_done());

        let c1 = state.next_chunk(&data).unwrap();
        assert_eq!(c1.len(), ONE28_K);

        let c2 = state.next_chunk(&data).unwrap();
        assert_eq!(c2.len(), ONE28_K);

        let c3 = state.next_chunk(&data).unwrap();
        assert_eq!(c3.len(), 300 * 1024 - 2 * ONE28_K);

        assert!(state.next_chunk(&data).is_none());
        assert!(state.is_done());
    }
}
