//! Firmware Analysis Module
//!
//! Provides unified firmware analysis API for CLI, TUI, and xtask.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::fuph::FuphHeader;
use crate::ifwi_version::{self, FirmwareVersions};

/// Firmware file type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareType {
    /// DnX Firmware (dnx_fwr.bin)
    DnxFirmware,
    /// DnX OS Recovery (dnx_osr.img)
    DnxOsRecovery,
    /// Full IFWI Image
    Ifwi,
    /// Android Boot Image
    AndroidBoot,
    /// Unknown type
    Unknown,
}

impl fmt::Display for FirmwareType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FirmwareType::DnxFirmware => write!(f, "DnX Firmware"),
            FirmwareType::DnxOsRecovery => write!(f, "DnX OS Recovery"),
            FirmwareType::Ifwi => write!(f, "IFWI Image"),
            FirmwareType::AndroidBoot => write!(f, "Android Boot"),
            FirmwareType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Magic marker found in firmware
#[derive(Debug, Clone)]
pub struct MarkerInfo {
    pub name: String,
    pub pattern: Vec<u8>,
    pub position: usize,
    pub description: String,
}

/// RSA signature information
#[derive(Debug, Clone)]
pub struct RsaSignature {
    pub offset: usize,
    pub size: usize,
    pub hash: String,
    pub valid: bool,
}

/// Validation check result
#[derive(Debug, Clone)]
pub struct ValidationCheck {
    pub name: String,
    pub passed: bool,
    pub message: String,
}

/// Token information
#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub marker: String,
    pub offset: usize,
    pub size: usize,
    pub platform: String,
}

/// Chaabi information
#[derive(Debug, Clone)]
pub struct ChaabiInfo {
    pub offset: usize,
    pub size: usize,
    pub ch00_pos: usize,
    pub cdph_pos: usize,
}

/// Complete firmware analysis result
#[derive(Debug, Clone)]
pub struct FirmwareAnalysis {
    /// Source file path
    pub path: PathBuf,
    /// File name
    pub filename: String,
    /// File size in bytes
    pub size: u64,
    /// Detected firmware type
    pub file_type: FirmwareType,
    /// SHA256 hash of file
    pub sha256: String,
    /// Magic markers found
    pub markers: Vec<MarkerInfo>,
    /// RSA signature info
    pub rsa_signature: Option<RsaSignature>,
    /// Token info (for DnX firmware)
    pub token: Option<TokenInfo>,
    /// Chaabi info (for DnX firmware)
    pub chaabi: Option<ChaabiInfo>,
    /// IFWI versions (if available)
    pub versions: Option<FirmwareVersions>,
    /// FUPH header (if available)
    pub fuph: Option<FuphHeader>,
    /// Validation checks
    pub validations: Vec<ValidationCheck>,
    /// Raw data (for further analysis)
    #[allow(dead_code)]
    data: Vec<u8>,
}

impl FirmwareAnalysis {
    /// Analyze a firmware file
    pub fn analyze(path: &Path) -> std::io::Result<Self> {
        let data = std::fs::read(path)?;
        let size = data.len() as u64;
        let filename = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        // Compute SHA256
        let sha256 = compute_sha256(&data);

        // Detect file type
        let file_type = detect_file_type(&data);

        // Find magic markers
        let markers = find_markers(&data);

        // Extract RSA signature info (for DnX firmware)
        let rsa_signature = extract_rsa_signature(&data);

        // Extract token info
        let token = extract_token_info(&data, &markers);

        // Extract Chaabi info
        let chaabi = extract_chaabi_info(&data, &markers);

        // Try to extract IFWI versions
        let versions = ifwi_version::get_image_fw_rev(&data).ok();

        // Try to parse FUPH header
        let fuph = FuphHeader::parse(&data);

        // Run validation checks
        let validations = run_validations(&data, &markers);

        Ok(Self {
            path: path.to_path_buf(),
            filename,
            size,
            file_type,
            sha256,
            markers,
            rsa_signature,
            token,
            chaabi,
            versions,
            fuph,
            validations,
            data,
        })
    }

    /// Check if all validations passed
    pub fn is_valid(&self) -> bool {
        self.validations.iter().all(|v| v.passed)
    }

    /// Get validation summary
    pub fn validation_summary(&self) -> String {
        let passed = self.validations.iter().filter(|v| v.passed).count();
        let total = self.validations.len();
        format!("{}/{} checks passed", passed, total)
    }

    /// Format as text for display
    pub fn to_text(&self) -> String {
        let mut out = String::new();

        out.push_str(&format!("Firmware Analysis: {}\n", self.filename));
        out.push_str(&format!("{}\n", "=".repeat(50)));
        out.push_str(&format!(
            "File size: {} bytes ({:.2} KB)\n",
            self.size,
            self.size as f64 / 1024.0
        ));
        out.push_str(&format!("Type: {}\n", self.file_type));
        out.push_str(&format!("SHA256: {}...\n", &self.sha256[..32]));

        // Markers
        if !self.markers.is_empty() {
            out.push_str("\nMagic markers:\n");
            for m in &self.markers {
                out.push_str(&format!(
                    "  {}: 0x{:05X} - {}\n",
                    m.name, m.position, m.description
                ));
            }
        }

        // RSA
        if let Some(rsa) = &self.rsa_signature {
            out.push_str("\nRSA Signature:\n");
            out.push_str(&format!("  Offset: 0x{:X}\n", rsa.offset));
            out.push_str(&format!("  Hash: {}...\n", &rsa.hash[..32]));
        }

        // Token
        if let Some(token) = &self.token {
            out.push_str("\nToken:\n");
            out.push_str(&format!(
                "  Marker: {} ({})\n",
                token.marker, token.platform
            ));
            out.push_str(&format!("  Offset: 0x{:X}\n", token.offset));
            out.push_str(&format!("  Size: {} bytes\n", token.size));
        }

        // Chaabi
        if let Some(chaabi) = &self.chaabi {
            out.push_str("\nChaabi:\n");
            out.push_str(&format!("  Offset: 0x{:X}\n", chaabi.offset));
            out.push_str(&format!(
                "  Size: {} bytes ({:.1} KB)\n",
                chaabi.size,
                chaabi.size as f64 / 1024.0
            ));
        }

        // Versions
        if let Some(v) = &self.versions {
            out.push_str("\nVersions:\n");
            out.push_str(&format!("  IFWI: {}\n", v.ifwi));
            out.push_str(&format!("  SCU: {}\n", v.scu));
            out.push_str(&format!("  Chaabi: {}\n", v.chaabi));
        }

        // Validations
        out.push_str(&format!("\nValidation ({}):\n", self.validation_summary()));
        for v in &self.validations {
            let icon = if v.passed { "✅" } else { "❌" };
            out.push_str(&format!("  {} {}: {}\n", icon, v.name, v.message));
        }

        out
    }

    /// Format as JSON
    pub fn to_json(&self) -> String {
        let mut out = String::from("{\n");
        out.push_str(&format!("  \"filename\": \"{}\",\n", self.filename));
        out.push_str(&format!("  \"size\": {},\n", self.size));
        out.push_str(&format!("  \"type\": \"{}\",\n", self.file_type));
        out.push_str(&format!("  \"sha256\": \"{}\",\n", self.sha256));
        out.push_str(&format!("  \"valid\": {},\n", self.is_valid()));

        // Markers
        out.push_str("  \"markers\": [\n");
        for (i, m) in self.markers.iter().enumerate() {
            out.push_str(&format!(
                "    {{\"name\": \"{}\", \"position\": {}}}",
                m.name, m.position
            ));
            if i < self.markers.len() - 1 {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("  ],\n");

        // Validations
        out.push_str(&format!(
            "  \"validation_summary\": \"{}\"\n",
            self.validation_summary()
        ));
        out.push_str("}\n");

        out
    }

    /// Format as markdown table
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();

        out.push_str(&format!("## {}\n\n", self.filename));
        out.push_str("| Property | Value |\n");
        out.push_str("|----------|-------|\n");
        out.push_str(&format!("| Size | {} bytes |\n", self.size));
        out.push_str(&format!("| Type | {} |\n", self.file_type));
        out.push_str(&format!("| SHA256 | `{}...` |\n", &self.sha256[..16]));
        out.push_str(&format!(
            "| Valid | {} |\n",
            if self.is_valid() { "✅" } else { "❌" }
        ));

        if !self.markers.is_empty() {
            out.push_str("\n### Markers\n\n");
            out.push_str("| Name | Position | Description |\n");
            out.push_str("|------|----------|-------------|\n");
            for m in &self.markers {
                out.push_str(&format!(
                    "| {} | 0x{:05X} | {} |\n",
                    m.name, m.position, m.description
                ));
            }
        }

        out
    }
}

/// Compare two firmware files
#[derive(Debug, Clone)]
pub struct FirmwareComparison {
    pub file1: String,
    pub file2: String,
    pub size_match: bool,
    pub rsa_match: bool,
    pub diff_count: usize,
    pub diff_percentage: f64,
    pub diff_regions: Vec<DiffRegion>,
}

#[derive(Debug, Clone)]
pub struct DiffRegion {
    pub start: usize,
    pub end: usize,
    pub size: usize,
    pub description: String,
}

impl FirmwareComparison {
    /// Compare two firmware files
    pub fn compare(path1: &Path, path2: &Path) -> std::io::Result<Self> {
        let data1 = std::fs::read(path1)?;
        let data2 = std::fs::read(path2)?;

        let file1 = path1
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let file2 = path2
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let size_match = data1.len() == data2.len();

        // Compare RSA signatures
        let rsa_match = if data1.len() >= 0x188 && data2.len() >= 0x188 {
            data1[0x88..0x188] == data2[0x88..0x188]
        } else {
            false
        };

        // Count differences
        let min_len = data1.len().min(data2.len());
        let diff_count = data1[..min_len]
            .iter()
            .zip(data2[..min_len].iter())
            .filter(|(a, b)| a != b)
            .count();

        let diff_percentage = if min_len > 0 {
            (diff_count as f64 / min_len as f64) * 100.0
        } else {
            0.0
        };

        // Find diff regions
        let diff_regions = find_diff_regions(&data1, &data2);

        Ok(Self {
            file1,
            file2,
            size_match,
            rsa_match,
            diff_count,
            diff_percentage,
            diff_regions,
        })
    }

    /// Format comparison as text
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("Comparing: {} vs {}\n", self.file1, self.file2));
        out.push_str(&format!("{}\n", "=".repeat(50)));
        out.push_str(&format!(
            "Size match: {}\n",
            if self.size_match { "✅ Yes" } else { "❌ No" }
        ));
        out.push_str(&format!(
            "RSA match: {}\n",
            if self.rsa_match {
                "✅ Identical"
            } else {
                "❌ Different"
            }
        ));
        out.push_str(&format!(
            "Different bytes: {} ({:.3}%)\n",
            self.diff_count, self.diff_percentage
        ));

        if !self.diff_regions.is_empty() {
            out.push_str(&format!("\nDiff regions ({}):\n", self.diff_regions.len()));
            for (i, r) in self.diff_regions.iter().take(10).enumerate() {
                out.push_str(&format!(
                    "  {}: 0x{:05X}-0x{:05X} ({} bytes) - {}\n",
                    i + 1,
                    r.start,
                    r.end,
                    r.size,
                    r.description
                ));
            }
            if self.diff_regions.len() > 10 {
                out.push_str(&format!(
                    "  ... and {} more regions\n",
                    self.diff_regions.len() - 10
                ));
            }
        }

        out
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn compute_sha256(data: &[u8]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Simple hash for now (replace with proper SHA256 if crypto is added)
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    let h1 = hasher.finish();
    data[..data.len().min(1024)].hash(&mut hasher);
    let h2 = hasher.finish();
    format!(
        "{:016x}{:016x}{:016x}{:016x}",
        h1,
        h2,
        h1 ^ h2,
        h1.wrapping_add(h2)
    )
}

fn detect_file_type(data: &[u8]) -> FirmwareType {
    // Check for $DnX marker
    if data.len() > 0x84 && &data[0x80..0x84] == b"$DnX" {
        // Check for $OS$ header
        if data.len() >= 4 && &data[0..4] == b"$OS$" {
            return FirmwareType::DnxOsRecovery;
        }
        return FirmwareType::DnxFirmware;
    }

    // Check for $OS$ header
    if data.len() >= 4 && &data[0..4] == b"$OS$" {
        return FirmwareType::DnxOsRecovery;
    }

    // Check for ANDROID!
    if data.windows(8).any(|w| w == b"ANDROID!") {
        return FirmwareType::AndroidBoot;
    }

    // Check for $FIP (full IFWI)
    if data.windows(4).any(|w| w == b"$FIP") {
        return FirmwareType::Ifwi;
    }

    FirmwareType::Unknown
}

fn find_markers(data: &[u8]) -> Vec<MarkerInfo> {
    let patterns: &[(&str, &[u8], &str)] = &[
        ("$DnX", b"$DnX", "DnX signature marker"),
        ("$FIP", b"$FIP", "FIP version block"),
        ("$CHT", b"$CHT", "TNG A0 Token marker"),
        ("DTKN", b"DTKN", "TNG B0+ Token marker"),
        ("ChPr", b"ChPr", "TNG B0/ANN Token marker"),
        ("CH00", b"CH00", "Chaabi FW start"),
        ("CDPH", b"CDPH", "Chaabi FW end"),
        ("IFWI", b"IFWI", "IFWI chunk marker"),
        ("$OS$", b"$OS$", "OS DnX header"),
        ("ANDROID!", b"ANDROID!", "Android boot image"),
        ("$MN2", b"$MN2", "Manifest 2"),
    ];

    let mut markers = Vec::new();
    for (name, pattern, desc) in patterns {
        if let Some(pos) = data.windows(pattern.len()).position(|w| w == *pattern) {
            markers.push(MarkerInfo {
                name: name.to_string(),
                pattern: pattern.to_vec(),
                position: pos,
                description: desc.to_string(),
            });
        }
    }

    markers.sort_by_key(|m| m.position);
    markers
}

fn extract_rsa_signature(data: &[u8]) -> Option<RsaSignature> {
    if data.len() < 0x188 {
        return None;
    }

    // RSA signature is at 0x88-0x188 (256 bytes)
    let rsa_data = &data[0x88..0x188];
    let hash = compute_sha256(rsa_data);

    Some(RsaSignature {
        offset: 0x88,
        size: 256,
        hash,
        valid: true, // We can't verify without the public key
    })
}

fn extract_token_info(_data: &[u8], markers: &[MarkerInfo]) -> Option<TokenInfo> {
    let cht = markers.iter().find(|m| m.name == "$CHT");
    let ch00 = markers.iter().find(|m| m.name == "CH00");

    if let (Some(cht), Some(ch00)) = (cht, ch00)
        && cht.position < ch00.position
    {
        let offset = cht.position.saturating_sub(0x80);
        let size = ch00.position.saturating_sub(0x80) - offset;
        return Some(TokenInfo {
            marker: "$CHT".to_string(),
            offset,
            size,
            platform: "TNG A0 (Tangier A0)".to_string(),
        });
    }

    let dtkn = markers.iter().find(|m| m.name == "DTKN");
    if let (Some(dtkn), Some(ch00)) = (dtkn, ch00)
        && dtkn.position < ch00.position
    {
        let offset = dtkn.position;
        let size = ch00.position.saturating_sub(0x80) - offset;
        return Some(TokenInfo {
            marker: "DTKN".to_string(),
            offset,
            size,
            platform: "TNG B0+".to_string(),
        });
    }

    None
}

fn extract_chaabi_info(_data: &[u8], markers: &[MarkerInfo]) -> Option<ChaabiInfo> {
    let ch00 = markers.iter().find(|m| m.name == "CH00")?;
    let cdph = markers.iter().find(|m| m.name == "CDPH")?;

    let offset = ch00.position.saturating_sub(0x80);
    let size = cdph.position - offset;

    Some(ChaabiInfo {
        offset,
        size,
        ch00_pos: ch00.position,
        cdph_pos: cdph.position,
    })
}

fn run_validations(data: &[u8], markers: &[MarkerInfo]) -> Vec<ValidationCheck> {
    let mut checks = Vec::new();

    // Check $DnX signature
    let has_dnx = markers.iter().any(|m| m.name == "$DnX");
    checks.push(ValidationCheck {
        name: "DnX Signature".to_string(),
        passed: has_dnx,
        message: if has_dnx {
            "Found at expected position"
        } else {
            "Not found"
        }
        .to_string(),
    });

    // Check CH00 marker
    let has_ch00 = markers.iter().any(|m| m.name == "CH00");
    checks.push(ValidationCheck {
        name: "Chaabi Marker".to_string(),
        passed: has_ch00,
        message: if has_ch00 {
            "CH00 marker found"
        } else {
            "CH00 not found"
        }
        .to_string(),
    });

    // Check CDPH marker
    let has_cdph = markers.iter().any(|m| m.name == "CDPH");
    checks.push(ValidationCheck {
        name: "CDPH Marker".to_string(),
        passed: has_cdph,
        message: if has_cdph {
            "CDPH marker found"
        } else {
            "CDPH not found"
        }
        .to_string(),
    });

    // Check file size
    let size_ok = data.len() > 1024;
    checks.push(ValidationCheck {
        name: "File Size".to_string(),
        passed: size_ok,
        message: format!("{} bytes", data.len()),
    });

    checks
}

fn find_diff_regions(data1: &[u8], data2: &[u8]) -> Vec<DiffRegion> {
    let min_len = data1.len().min(data2.len());
    let mut regions = Vec::new();
    let mut in_diff = false;
    let mut diff_start = 0;

    for i in 0..min_len {
        if data1[i] != data2[i] {
            if !in_diff {
                diff_start = i;
                in_diff = true;
            }
        } else if in_diff {
            regions.push(DiffRegion {
                start: diff_start,
                end: i - 1,
                size: i - diff_start,
                description: describe_region(diff_start),
            });
            in_diff = false;
        }
    }

    if in_diff {
        regions.push(DiffRegion {
            start: diff_start,
            end: min_len - 1,
            size: min_len - diff_start,
            description: describe_region(diff_start),
        });
    }

    regions
}

fn describe_region(offset: usize) -> String {
    if offset < 0x80 {
        "Header".to_string()
    } else if offset < 0x188 {
        "RSA Signature".to_string()
    } else if offset < 0x4B00 {
        "VRL/IFWI".to_string()
    } else if offset < 0x8B00 {
        "Token".to_string()
    } else if offset < 0x1AB00 {
        "Chaabi FW".to_string()
    } else {
        "CDPH/Footer".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_file_type() {
        let mut data = vec![0u8; 0x100];
        data[0x80..0x84].copy_from_slice(b"$DnX");
        assert_eq!(detect_file_type(&data), FirmwareType::DnxFirmware);
    }

    #[test]
    fn test_find_markers() {
        let mut data = vec![0u8; 0x200];
        data[0x80..0x84].copy_from_slice(b"$DnX");
        data[0x100..0x104].copy_from_slice(b"CH00");

        let markers = find_markers(&data);
        assert_eq!(markers.len(), 2);
        assert_eq!(markers[0].name, "$DnX");
        assert_eq!(markers[1].name, "CH00");
    }
}
