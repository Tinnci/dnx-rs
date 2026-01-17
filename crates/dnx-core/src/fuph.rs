//! FUPH (Firmware Update Payload Header) Parser
//!
//! Based on Intel SCU Firmware Update Driver (fw_update.c)
//! This module parses the FUPH header at the end of IFWI images.

use std::fmt;

/// FUPH Header magic string
pub const FUPH_MAGIC: &[u8] = b"UPH$";

/// FUPH Header size (standard)
pub const FUPH_HDR_LEN: usize = 36;

/// FUPH Header offsets
pub const FUPH_MIP_OFFSET: usize = 0x04;
pub const FUPH_IFWI_OFFSET: usize = 0x08;
pub const FUPH_PSFW1_OFFSET: usize = 0x0c;
pub const FUPH_PSFW2_OFFSET: usize = 0x10;
pub const FUPH_SSFW_OFFSET: usize = 0x14;
pub const FUPH_SUCP_OFFSET: usize = 0x18;
pub const FUPH_VEDFW_OFFSET: usize = 0x1c;

/// DNX Header length
pub const DNX_HDR_LEN: usize = 24;

/// DNX Header offsets
pub const DNX_SIZE_OFFSET: usize = 0;
pub const DNX_GP_FLAG_OFFSET: usize = 4;
pub const DNX_XOR_CHK_OFFSET: usize = 20;

/// Firmware component request strings (from kernel driver)
pub mod requests {
    pub const DNX_IMAGE: &str = "DXBL";
    pub const FUPH_HDR_SIZE: &str = "RUPHS";
    pub const FUPH: &str = "RUPH";
    pub const MIP: &str = "DMIP";
    pub const IFWI: &str = "IFW";
    pub const LOWER_128K: &str = "LOFW";
    pub const UPPER_128K: &str = "HIFW";
    pub const PSFW1: &str = "PSFW1";
    pub const PSFW2: &str = "PSFW2";
    pub const SSFW: &str = "SSFW";
    pub const SUCP: &str = "SuCP";
    pub const VEDFW: &str = "VEDFW";
    pub const UPDATE_DONE: &str = "HLT$";
    pub const UPDATE_ABORT: &str = "HLT0";
    pub const UPDATE_ERROR: &str = "ER";
}

/// FUPH Header attributes - sizes of firmware components
#[derive(Debug, Clone, Default)]
pub struct FuphHeader {
    /// Header length (28 or 36 bytes)
    pub header_len: usize,
    /// MIP (Minimum Information Partition) size in bytes
    pub mip_size: u32,
    /// IFWI size in bytes  
    pub ifwi_size: u32,
    /// Primary Security Firmware 1 size
    pub psfw1_size: u32,
    /// Primary Security Firmware 2 size
    pub psfw2_size: u32,
    /// Secondary Security Firmware size
    pub ssfw_size: u32,
    /// SCU Patch size
    pub sucp_size: u32,
    /// Video Encode/Decode Firmware size
    pub vedfw_size: u32,
}

impl FuphHeader {
    /// Parse FUPH header from the end of firmware data
    pub fn parse(data: &[u8]) -> Option<Self> {
        // Find FUPH magic by scanning backwards
        let header_len = find_fuph_header_len(data)?;

        if data.len() < header_len {
            return None;
        }

        let fuph_start = data.len() - header_len;
        let fuph_data = &data[fuph_start..];

        // Sizes are stored as DWORDs (multiply by 4 to get bytes)
        let read_size = |offset: usize| -> u32 {
            if offset + 4 <= fuph_data.len() {
                u32::from_le_bytes([
                    fuph_data[offset],
                    fuph_data[offset + 1],
                    fuph_data[offset + 2],
                    fuph_data[offset + 3],
                ]) * 4
            } else {
                0
            }
        };

        Some(FuphHeader {
            header_len,
            mip_size: read_size(FUPH_MIP_OFFSET),
            ifwi_size: read_size(FUPH_IFWI_OFFSET),
            psfw1_size: read_size(FUPH_PSFW1_OFFSET),
            psfw2_size: read_size(FUPH_PSFW2_OFFSET),
            ssfw_size: read_size(FUPH_SSFW_OFFSET),
            sucp_size: read_size(FUPH_SUCP_OFFSET),
            vedfw_size: if header_len >= FUPH_HDR_LEN {
                read_size(FUPH_VEDFW_OFFSET)
            } else {
                0
            },
        })
    }

    /// Total firmware size
    pub fn total_size(&self) -> u32 {
        self.mip_size
            + self.ifwi_size
            + self.psfw1_size
            + self.psfw2_size
            + self.ssfw_size
            + self.sucp_size
            + self.vedfw_size
    }
}

impl fmt::Display for FuphHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "FUPH Header (len={}):", self.header_len)?;
        writeln!(
            f,
            "  MIP:    {:>8} bytes ({:.2} KB)",
            self.mip_size,
            self.mip_size as f64 / 1024.0
        )?;
        writeln!(
            f,
            "  IFWI:   {:>8} bytes ({:.2} KB)",
            self.ifwi_size,
            self.ifwi_size as f64 / 1024.0
        )?;
        writeln!(
            f,
            "  PSFW1:  {:>8} bytes ({:.2} KB)",
            self.psfw1_size,
            self.psfw1_size as f64 / 1024.0
        )?;
        writeln!(
            f,
            "  PSFW2:  {:>8} bytes ({:.2} KB)",
            self.psfw2_size,
            self.psfw2_size as f64 / 1024.0
        )?;
        writeln!(
            f,
            "  SSFW:   {:>8} bytes ({:.2} KB)",
            self.ssfw_size,
            self.ssfw_size as f64 / 1024.0
        )?;
        writeln!(
            f,
            "  SUCP:   {:>8} bytes ({:.2} KB)",
            self.sucp_size,
            self.sucp_size as f64 / 1024.0
        )?;
        writeln!(
            f,
            "  VEDFW:  {:>8} bytes ({:.2} KB)",
            self.vedfw_size,
            self.vedfw_size as f64 / 1024.0
        )?;
        writeln!(
            f,
            "  Total:  {:>8} bytes ({:.2} MB)",
            self.total_size(),
            self.total_size() as f64 / 1024.0 / 1024.0
        )
    }
}

/// DNX Header structure for firmware update
#[derive(Debug, Clone)]
pub struct DnxHeader {
    /// DNX payload size
    pub size: u32,
    /// General Purpose flags
    pub gp_flags: u32,
    /// Reserved fields
    pub reserved: [u32; 3],
    /// XOR checksum (size ^ gp_flags)
    pub xor_checksum: u32,
}

impl DnxHeader {
    /// Create a new DNX header
    pub fn new(size: u32, gp_flags: u32) -> Self {
        Self {
            size,
            gp_flags,
            reserved: [0; 3],
            xor_checksum: size ^ gp_flags,
        }
    }

    /// Parse DNX header from bytes
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < DNX_HDR_LEN {
            return None;
        }

        let read_u32 = |offset: usize| -> u32 {
            u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ])
        };

        Some(Self {
            size: read_u32(DNX_SIZE_OFFSET),
            gp_flags: read_u32(DNX_GP_FLAG_OFFSET),
            reserved: [read_u32(8), read_u32(12), read_u32(16)],
            xor_checksum: read_u32(DNX_XOR_CHK_OFFSET),
        })
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> [u8; DNX_HDR_LEN] {
        let mut bytes = [0u8; DNX_HDR_LEN];
        bytes[0..4].copy_from_slice(&self.size.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.gp_flags.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.reserved[0].to_le_bytes());
        bytes[12..16].copy_from_slice(&self.reserved[1].to_le_bytes());
        bytes[16..20].copy_from_slice(&self.reserved[2].to_le_bytes());
        bytes[20..24].copy_from_slice(&self.xor_checksum.to_le_bytes());
        bytes
    }

    /// Validate XOR checksum
    pub fn is_valid(&self) -> bool {
        self.xor_checksum == (self.size ^ self.gp_flags)
    }
}

/// Find FUPH header length by scanning backwards for "UPH$" magic
fn find_fuph_header_len(data: &[u8]) -> Option<usize> {
    const SKIP_BYTES: usize = 8;
    const FUPH_MAX_LEN: usize = 36;

    if data.len() < SKIP_BYTES + 4 {
        return None;
    }

    // Start from end minus skip bytes, scan backwards
    let mut offset = data.len() - SKIP_BYTES;
    let mut cnt = 0usize;

    while cnt <= FUPH_MAX_LEN {
        if offset < 4 {
            break;
        }

        if &data[offset - 4..offset] == FUPH_MAGIC {
            return Some(cnt + SKIP_BYTES);
        }

        offset = offset.saturating_sub(4);
        cnt += 4;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dnx_header_create() {
        let header = DnxHeader::new(109812, 0);
        assert_eq!(header.size, 109812);
        assert_eq!(header.gp_flags, 0);
        assert_eq!(header.xor_checksum, 109812);
        assert!(header.is_valid());
    }

    #[test]
    fn test_dnx_header_roundtrip() {
        let header = DnxHeader::new(12345, 0x80000000);
        let bytes = header.to_bytes();
        let parsed = DnxHeader::parse(&bytes).unwrap();
        assert_eq!(header.size, parsed.size);
        assert_eq!(header.gp_flags, parsed.gp_flags);
        assert_eq!(header.xor_checksum, parsed.xor_checksum);
    }
}
