//! OS image parsing and OSIP handling.
//!
//! Handles OS recovery images with OSIP (OS Image Package) structure.
//! Reference: xFSTK `dldrstate.cpp` OsHandleROSIP, OsHandleRIMG

use crate::protocol::constants::OSIP_PARTITIONTABLE_SIZE;
use crate::protocol::header::{HeaderError, OsipHeader};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OsImageError {
    #[error("File too small: {actual} bytes, minimum {minimum}")]
    FileTooSmall { actual: usize, minimum: usize },
    #[error("Invalid OSIP signature: 0x{actual:08X}")]
    InvalidSignature { actual: u32 },
    #[error("Header error: {0}")]
    Header(#[from] HeaderError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Partition {index} out of range")]
    PartitionOutOfRange { index: usize },
}

/// OSIP signature constant.
pub const OSIP_SIGNATURE: u32 = 0x24534F24; // '$OS$'

/// Parsed OS image with OSIP support.
#[derive(Debug)]
pub struct OsImage {
    /// Raw image data
    data: Vec<u8>,
    /// Parsed OSIP header
    #[allow(dead_code)]
    osip: OsipHeader,
    /// Number of OS partitions
    num_partitions: usize,
    /// Partition info (offset, size) pairs
    partitions: Vec<(usize, usize)>,
}

impl OsImage {
    /// Parse OS image from raw bytes.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, OsImageError> {
        if data.len() < OSIP_PARTITIONTABLE_SIZE {
            return Err(OsImageError::FileTooSmall {
                actual: data.len(),
                minimum: OSIP_PARTITIONTABLE_SIZE,
            });
        }

        let osip = OsipHeader::from_bytes(&data)?;

        // Validate signature if present (some images may not have it)
        // $OS$ = 0x24534F24
        if osip.signature != 0 && osip.signature != OSIP_SIGNATURE {
            // Not a critical error, just log it
            tracing::warn!(
                signature = format!("0x{:08X}", osip.signature),
                "Non-standard OSIP signature"
            );
        }

        let num_partitions = osip.num_pointers as usize;

        // Parse partition entries
        let mut partitions = Vec::with_capacity(num_partitions);
        for i in 0..num_partitions {
            if let Some(size) = osip.os_partition_size(i) {
                // Offset calculation: each partition entry is at offset 0x30 + i * 0x18
                // The actual data offset would need to be read from the entry
                // For simplicity, we assume sequential layout after OSIP header
                let offset =
                    OSIP_PARTITIONTABLE_SIZE + partitions.iter().map(|(_, s)| *s).sum::<usize>();
                partitions.push((offset, size as usize));
            }
        }

        Ok(Self {
            data,
            osip,
            num_partitions,
            partitions,
        })
    }

    /// Get OSIP header bytes (512 bytes).
    pub fn osip_bytes(&self) -> &[u8] {
        &self.data[..OSIP_PARTITIONTABLE_SIZE.min(self.data.len())]
    }

    /// Get OSIP size as u32 for sending.
    pub fn osip_size(&self) -> u32 {
        OSIP_PARTITIONTABLE_SIZE as u32
    }

    /// Get number of partitions.
    pub fn num_partitions(&self) -> usize {
        self.num_partitions
    }

    /// Get partition data by index.
    pub fn partition(&self, index: usize) -> Result<&[u8], OsImageError> {
        if index >= self.partitions.len() {
            return Err(OsImageError::PartitionOutOfRange { index });
        }

        let (offset, size) = self.partitions[index];
        if offset + size > self.data.len() {
            return Err(OsImageError::FileTooSmall {
                actual: self.data.len(),
                minimum: offset + size,
            });
        }

        Ok(&self.data[offset..offset + size])
    }

    /// Get chunk iterator for partition.
    pub fn partition_chunks(
        &self,
        index: usize,
        chunk_size: usize,
    ) -> Result<OsChunkIterator<'_>, OsImageError> {
        let data = self.partition(index)?;
        Ok(OsChunkIterator::new(data, chunk_size))
    }

    /// Get all image data after OSIP header.
    pub fn image_data(&self) -> &[u8] {
        if self.data.len() <= OSIP_PARTITIONTABLE_SIZE {
            return &[];
        }
        &self.data[OSIP_PARTITIONTABLE_SIZE..]
    }

    /// Get chunk iterator for entire image (excluding OSIP header).
    pub fn image_chunks(&self, chunk_size: usize) -> OsChunkIterator<'_> {
        OsChunkIterator::new(self.image_data(), chunk_size)
    }

    /// Get raw data.
    pub fn raw_data(&self) -> &[u8] {
        &self.data
    }

    /// Total size.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// OS image chunk iterator.
#[derive(Debug)]
pub struct OsChunkIterator<'a> {
    data: &'a [u8],
    chunk_size: usize,
    offset: usize,
    total_chunks: usize,
    current_chunk: usize,
}

impl<'a> OsChunkIterator<'a> {
    pub fn new(data: &'a [u8], chunk_size: usize) -> Self {
        let total_chunks = if data.is_empty() {
            0
        } else {
            data.len().div_ceil(chunk_size)
        };

        Self {
            data,
            chunk_size,
            offset: 0,
            total_chunks,
            current_chunk: 0,
        }
    }

    /// Total number of chunks.
    pub fn total(&self) -> usize {
        self.total_chunks
    }

    /// Current chunk index.
    pub fn current(&self) -> usize {
        self.current_chunk
    }

    /// Remaining bytes.
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.offset)
    }

    /// Progress percentage.
    pub fn progress_pct(&self) -> u8 {
        if self.total_chunks == 0 {
            100
        } else {
            ((self.current_chunk * 100) / self.total_chunks) as u8
        }
    }

    /// Reset iterator.
    pub fn reset(&mut self) {
        self.offset = 0;
        self.current_chunk = 0;
    }
}

impl<'a> Iterator for OsChunkIterator<'a> {
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

/// OS chunk state for stateful sending.
#[derive(Debug, Default, Clone)]
pub struct OsChunkState {
    pub current: usize,
    pub total: usize,
    pub offset: usize,
    pub chunk_size: usize,
    pub data_size: usize,
}

impl OsChunkState {
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

    pub fn is_done(&self) -> bool {
        self.current >= self.total
    }

    pub fn reset(&mut self) {
        self.current = 0;
        self.offset = 0;
    }

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
    fn test_os_chunk_iterator() {
        let data = vec![0u8; 1024 * 1024]; // 1MB
        let chunk_size = 64 * 1024; // 64KB chunks

        let iter = OsChunkIterator::new(&data, chunk_size);
        assert_eq!(iter.total(), 16);

        let chunks: Vec<_> = OsChunkIterator::new(&data, chunk_size).collect();
        assert_eq!(chunks.len(), 16);
        assert!(chunks.iter().all(|c| c.len() == chunk_size));
    }

    #[test]
    fn test_os_chunk_state() {
        let data = vec![1u8; 150 * 1024]; // 150KB
        let chunk_size = 64 * 1024;
        let mut state = OsChunkState::new(data.len(), chunk_size);

        assert_eq!(state.total, 3); // 64 + 64 + 22

        let c1 = state.next_chunk(&data).unwrap();
        assert_eq!(c1.len(), chunk_size);

        let c2 = state.next_chunk(&data).unwrap();
        assert_eq!(c2.len(), chunk_size);

        let c3 = state.next_chunk(&data).unwrap();
        assert_eq!(c3.len(), 150 * 1024 - 2 * chunk_size);

        assert!(state.next_chunk(&data).is_none());
        assert!(state.is_done());
    }
}
