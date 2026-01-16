//! Data structure headers for DnX protocol.

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::Cursor;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HeaderError {
    #[error("Buffer too small: expected {expected}, got {actual}")]
    BufferTooSmall { expected: usize, actual: usize },
    #[error("Invalid magic: expected 0x{expected:08X}, got 0x{actual:08X}")]
    InvalidMagic { expected: u32, actual: u32 },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// DnX Header (24 bytes / 0x18)
///
/// Sent at the start of FW/OS download to specify size and checksum.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct DnxHeader {
    pub size: u32,
    pub checksum: u32,
    pub reserved: [u32; 4],
}

impl DnxHeader {
    pub const SIZE: usize = 24;

    pub fn new(size: u32, checksum: u32) -> Self {
        Self {
            size,
            checksum,
            reserved: [0; 4],
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(Self::SIZE);
        buf.write_u32::<LittleEndian>(self.size).unwrap();
        buf.write_u32::<LittleEndian>(self.checksum).unwrap();
        for &r in &self.reserved {
            buf.write_u32::<LittleEndian>(r).unwrap();
        }
        buf
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, HeaderError> {
        if data.len() < Self::SIZE {
            return Err(HeaderError::BufferTooSmall {
                expected: Self::SIZE,
                actual: data.len(),
            });
        }
        let mut cursor = Cursor::new(data);
        Ok(Self {
            size: cursor.read_u32::<LittleEndian>()?,
            checksum: cursor.read_u32::<LittleEndian>()?,
            reserved: [
                cursor.read_u32::<LittleEndian>()?,
                cursor.read_u32::<LittleEndian>()?,
                cursor.read_u32::<LittleEndian>()?,
                cursor.read_u32::<LittleEndian>()?,
            ],
        })
    }
}

/// FW Update Profile Header (variable size: 0x1C / 0x20 / 0x24)
///
/// Contains sizes for different firmware components.
#[derive(Debug, Clone, Default)]
pub struct FwUpdateProfileHeader {
    /// Raw header bytes
    pub data: Vec<u8>,
    /// Header size (0x1C, 0x20, or 0x24)
    pub size: usize,
}

impl FwUpdateProfileHeader {
    /// D0 platform header size
    pub const D0_SIZE: usize = 0x24;
    /// C0 platform header size
    pub const C0_SIZE: usize = 0x20;
    /// Old Medfield header size
    pub const OLD_MFD_SIZE: usize = 0x1C;

    pub fn from_firmware_image(fw_data: &[u8], header_size: usize) -> Result<Self, HeaderError> {
        if fw_data.len() < header_size {
            return Err(HeaderError::BufferTooSmall {
                expected: header_size,
                actual: fw_data.len(),
            });
        }
        Ok(Self {
            data: fw_data[..header_size].to_vec(),
            size: header_size,
        })
    }

    /// Get PSFW1 size from header.
    pub fn psfw1_size(&self) -> Option<u32> {
        self.read_u32_at(0x0C)
    }

    /// Get PSFW2 size from header.
    pub fn psfw2_size(&self) -> Option<u32> {
        self.read_u32_at(0x10)
    }

    /// Get SSFW size from header.
    pub fn ssfw_size(&self) -> Option<u32> {
        self.read_u32_at(0x14)
    }

    /// Get ROM Patch size from header.
    pub fn rom_patch_size(&self) -> Option<u32> {
        if self.size > 0x18 {
            self.read_u32_at(0x18)
        } else {
            None
        }
    }

    fn read_u32_at(&self, offset: usize) -> Option<u32> {
        if self.data.len() >= offset + 4 {
            let mut cursor = Cursor::new(&self.data[offset..]);
            cursor.read_u32::<LittleEndian>().ok()
        } else {
            None
        }
    }

    pub fn to_bytes(&self) -> &[u8] {
        &self.data
    }
}

/// OSIP (OS Image Package) Partition Table Header.
///
/// 512 bytes (0x200).
#[derive(Debug, Clone)]
pub struct OsipHeader {
    pub data: Vec<u8>,
    pub signature: u32,
    pub header_size: u32,
    pub num_pointers: u32,
}

impl OsipHeader {
    pub const SIZE: usize = 0x200;

    pub fn from_bytes(data: &[u8]) -> Result<Self, HeaderError> {
        if data.len() < Self::SIZE {
            return Err(HeaderError::BufferTooSmall {
                expected: Self::SIZE,
                actual: data.len(),
            });
        }
        let mut cursor = Cursor::new(data);
        let signature = cursor.read_u32::<LittleEndian>()?;
        let header_size = cursor.read_u32::<LittleEndian>()?;
        let num_pointers = cursor.read_u32::<LittleEndian>()?;

        Ok(Self {
            data: data[..Self::SIZE].to_vec(),
            signature,
            header_size,
            num_pointers,
        })
    }

    /// Get size of OS partition N.
    pub fn os_partition_size(&self, n: usize) -> Option<u32> {
        let offset = (n * 0x18) + 0x30;
        if self.data.len() >= offset + 4 {
            let mut cursor = Cursor::new(&self.data[offset..]);
            cursor.read_u32::<LittleEndian>().ok()
        } else {
            None
        }
    }

    pub fn to_bytes(&self) -> &[u8] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dnx_header_roundtrip() {
        let header = DnxHeader::new(0x12345678, 0xDEADBEEF);
        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), DnxHeader::SIZE);

        let parsed = DnxHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.size, 0x12345678);
        assert_eq!(parsed.checksum, 0xDEADBEEF);
    }
}
