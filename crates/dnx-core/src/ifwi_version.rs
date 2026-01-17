//! IFWI Version Dump - Rust Implementation
//!
//! This module extracts firmware version information from Intel IFWI images.
//! Ported from: https://github.com/updateing/IFWIVersionDump
//!
//! Original code is from AOSP fugu device tree, licensed under Apache 2.0.

use std::fmt;
use std::io::{self, Read};

/// FIP_PATTERN: "$FIP" little-endian (inversed)
const FIP_PATTERN: u32 = 0x50494624;

/// Version pair (major, minor)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
}

impl Version {
    pub fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    pub fn is_valid(&self) -> bool {
        self.major != 0 || self.minor != 0
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04X}.{:04X}", self.major, self.minor)
    }
}

/// Complete firmware versions extracted from IFWI image
#[derive(Debug, Clone, Default)]
pub struct FirmwareVersions {
    /// IFWI overall version
    pub ifwi: Version,
    /// SCU (System Control Unit) version
    pub scu: Version,
    /// SCU Bootstrap version
    pub scu_bootstrap: Version,
    /// IA32 firmware version
    pub ia32: Version,
    /// Validation hooks / OEM version
    pub valhooks: Version,
    /// Chaabi (CH00) version
    pub chaabi: Version,
    /// mIA version
    pub mia: Version,
}

impl FirmwareVersions {
    /// Pretty print the firmware versions
    pub fn dump(&self) {
        println!("Image FW versions:");
        println!("       ifwi: {}", self.ifwi);
        println!("---- components ----");
        println!("        scu: {}", self.scu);
        println!("  hooks/oem: {}", self.valhooks);
        println!("       ia32: {}", self.ia32);
        println!("     chaabi: {}", self.chaabi);
        println!("        mIA: {}", self.mia);
    }

    /// Format as markdown table
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("| Component | Version |\n");
        out.push_str("|-----------|----------|\n");
        out.push_str(&format!("| IFWI | {} |\n", self.ifwi));
        out.push_str(&format!("| SCU | {} |\n", self.scu));
        out.push_str(&format!("| Hooks/OEM | {} |\n", self.valhooks));
        out.push_str(&format!("| IA32 | {} |\n", self.ia32));
        out.push_str(&format!("| Chaabi | {} |\n", self.chaabi));
        out.push_str(&format!("| mIA | {} |\n", self.mia));
        out
    }
}

/// FIP version block structure (8 bytes)
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
struct FipVersionBlock {
    minor: u16,
    major: u16,
    checksum: u8,
    reserved8: u8,
    reserved16: u16,
}

impl FipVersionBlock {
    fn as_version(&self) -> Version {
        Version {
            major: self.major,
            minor: self.minor,
        }
    }
}

/// FIP version block with size/dest (12 bytes, for CHxx components)
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
struct FipVersionBlockChxx {
    minor: u16,
    major: u16,
    checksum: u8,
    reserved8: u8,
    reserved16: u16,
    size: u16,
    dest: u16,
}

/// FIP Header structure (complete)
/// Total size: 4 + 8*18 + 12*15 + 8*4 = 4 + 144 + 180 + 32 = 360 bytes
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct FipHeader {
    fip_sig: u32,
    umip_rev: FipVersionBlock,
    spat_rev: FipVersionBlock,
    spct_rev: FipVersionBlock,
    rpch_rev: FipVersionBlock,
    ch00_rev: FipVersionBlock,
    mipd_rev: FipVersionBlock,
    mipn_rev: FipVersionBlock,
    scuc_rev: FipVersionBlock,
    hvm_rev: FipVersionBlock,
    mia_rev: FipVersionBlock,
    ia32_rev: FipVersionBlock,
    oem_rev: FipVersionBlock,
    ved_rev: FipVersionBlock,
    vec_rev: FipVersionBlock,
    mos_rev: FipVersionBlock,
    pos_rev: FipVersionBlock,
    cos_rev: FipVersionBlock,
    ch01_rev: FipVersionBlockChxx,
    ch02_rev: FipVersionBlockChxx,
    ch03_rev: FipVersionBlockChxx,
    ch04_rev: FipVersionBlockChxx,
    ch05_rev: FipVersionBlockChxx,
    ch06_rev: FipVersionBlockChxx,
    ch07_rev: FipVersionBlockChxx,
    ch08_rev: FipVersionBlockChxx,
    ch09_rev: FipVersionBlockChxx,
    ch10_rev: FipVersionBlockChxx,
    ch11_rev: FipVersionBlockChxx,
    ch12_rev: FipVersionBlockChxx,
    ch13_rev: FipVersionBlockChxx,
    ch14_rev: FipVersionBlockChxx,
    ch15_rev: FipVersionBlockChxx,
    dnx_rev: FipVersionBlock,
    reserved0_rev: FipVersionBlock,
    reserved1_rev: FipVersionBlock,
    ifwi_rev: FipVersionBlock,
}

impl Default for FipHeader {
    fn default() -> Self {
        // Safety: FipHeader is all primitive types, zeroed is valid
        unsafe { std::mem::zeroed() }
    }
}

/// Error type for IFWI parsing
#[derive(Debug)]
pub enum IfwiError {
    IoError(io::Error),
    FipNotFound,
    InvalidData(String),
}

impl From<io::Error> for IfwiError {
    fn from(e: io::Error) -> Self {
        IfwiError::IoError(e)
    }
}

impl fmt::Display for IfwiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IfwiError::IoError(e) => write!(f, "IO error: {}", e),
            IfwiError::FipNotFound => write!(f, "Couldn't find FIP magic in image"),
            IfwiError::InvalidData(s) => write!(f, "Invalid data: {}", s),
        }
    }
}

impl std::error::Error for IfwiError {}

/// Extract firmware versions from IFWI image data
pub fn get_image_fw_rev(data: &[u8]) -> Result<FirmwareVersions, IfwiError> {
    let mut versions = FirmwareVersions::default();
    let mut offset = 0;
    let fip_size = std::mem::size_of::<FipHeader>();
    let mut magic_found = false;

    while offset + fip_size <= data.len() {
        // Scan for FIP magic
        while offset + 4 <= data.len() {
            let magic = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);

            if magic == FIP_PATTERN {
                magic_found = true;
                break;
            }
            offset += 4;
        }

        if !magic_found {
            break;
        }

        if offset + fip_size > data.len() {
            break;
        }

        // Parse FIP header
        // Safety: We verified there's enough data, and FipHeader is packed C struct
        let fip: FipHeader =
            unsafe { std::ptr::read_unaligned(data[offset..].as_ptr() as *const FipHeader) };

        // Update versions (don't update if null)
        let scuc = fip.scuc_rev.as_version();
        if scuc.minor != 0 {
            versions.scu.minor = scuc.minor;
        }
        if scuc.major != 0 {
            versions.scu.major = scuc.major;
        }

        let ia32 = fip.ia32_rev.as_version();
        if ia32.minor != 0 {
            versions.ia32.minor = ia32.minor;
        }
        if ia32.major != 0 {
            versions.ia32.major = ia32.major;
        }

        let oem = fip.oem_rev.as_version();
        if oem.minor != 0 {
            versions.valhooks.minor = oem.minor;
        }
        if oem.major != 0 {
            versions.valhooks.major = oem.major;
        }

        let ifwi = fip.ifwi_rev.as_version();
        if ifwi.minor != 0 {
            versions.ifwi.minor = ifwi.minor;
        }
        if ifwi.major != 0 {
            versions.ifwi.major = ifwi.major;
        }

        let ch00 = fip.ch00_rev.as_version();
        if ch00.minor != 0 {
            versions.chaabi.minor = ch00.minor;
        }
        if ch00.major != 0 {
            versions.chaabi.major = ch00.major;
        }

        let mia = fip.mia_rev.as_version();
        if mia.minor != 0 {
            versions.mia.minor = mia.minor;
        }
        if mia.major != 0 {
            versions.mia.major = mia.major;
        }

        offset += 4;
        magic_found = false;
    }

    if !versions.ifwi.is_valid() && !versions.scu.is_valid() {
        return Err(IfwiError::FipNotFound);
    }

    Ok(versions)
}

/// Check IFWI file and print versions
pub fn check_ifwi_file(data: &[u8]) -> Result<FirmwareVersions, IfwiError> {
    let versions = get_image_fw_rev(data)?;
    versions.dump();
    Ok(versions)
}

/// Load and check IFWI file from path
pub fn check_ifwi_path(path: &std::path::Path) -> Result<FirmwareVersions, IfwiError> {
    let mut file = std::fs::File::open(path)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    check_ifwi_file(&data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_display() {
        let v = Version::new(0x0094, 0x0171);
        assert_eq!(format!("{}", v), "0094.0171");
    }

    #[test]
    fn test_fip_pattern() {
        assert_eq!(FIP_PATTERN, 0x50494624);
        let bytes = FIP_PATTERN.to_le_bytes();
        assert_eq!(&bytes, b"$FIP");
    }
}
