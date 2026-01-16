//! ACK code parsing and matching.
//!
//! DnX protocol uses variable-length ACK codes (4-7 bytes).
//! This module provides type-safe parsing and matching.

use std::fmt;

/// Parsed ACK code from device.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AckCode {
    /// Raw bytes of the ACK (up to 8 bytes, stored as u64)
    value: u64,
    /// Number of significant bytes
    len: u8,
}

impl AckCode {
    /// Create ACK code from raw bytes.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let len = bytes.len().min(8) as u8;
        let mut value: u64 = 0;
        for (i, &b) in bytes.iter().take(8).enumerate() {
            value |= (b as u64) << ((7 - i) * 8);
        }
        // Shift to align to MSB based on actual length
        let shift = (8 - len) * 8;
        value >>= shift;
        Self { value, len }
    }

    /// Create ACK from a 4-byte u32 constant.
    pub const fn from_u32(v: u32) -> Self {
        Self {
            value: v as u64,
            len: 4,
        }
    }

    /// Create ACK from a u64 constant (variable length).
    /// Determines length by counting significant bytes.
    pub const fn from_u64(v: u64) -> Self {
        // Count significant bytes
        let len = if v > 0x00FFFFFFFFFFFF {
            8
        } else if v > 0x0000FFFFFFFFFF {
            7
        } else if v > 0x000000FFFFFFFF {
            6
        } else if v > 0x00000000FFFFFF {
            5
        } else if v > 0x0000000000FFFF {
            4
        } else if v > 0x000000000000FF {
            3
        } else if v > 0x00000000000000 {
            2
        } else {
            1
        };
        Self { value: v, len }
    }

    /// Get ASCII representation if printable.
    pub fn as_ascii(&self) -> String {
        let bytes = self.value.to_be_bytes();
        let start = 8 - self.len as usize;
        bytes[start..]
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
            .collect()
    }

    /// Raw value.
    pub fn value(&self) -> u64 {
        self.value
    }

    /// Byte length.
    pub fn len(&self) -> u8 {
        self.len
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Check if this matches a u32 constant (common 4-byte ACKs).
    pub fn matches_u32(&self, expected: u32) -> bool {
        self.len >= 4 && (self.value & 0xFFFFFFFF) == expected as u64
    }

    /// Check if this matches a u64 constant (5+ byte ACKs like RUPHS, PSFW1).
    pub fn matches_u64(&self, expected: u64) -> bool {
        self.value == expected
    }

    /// Check if this is an error code (starts with 'ER').
    pub fn is_error(&self) -> bool {
        let be = self.value.to_be_bytes();
        let start = 8 - self.len as usize;
        self.len >= 4 && be[start] == b'E' && be[start + 1] == b'R'
    }
}

impl fmt::Debug for AckCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AckCode({:0width$X} '{}')",
            self.value,
            self.as_ascii(),
            width = (self.len * 2) as usize
        )
    }
}

impl fmt::Display for AckCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ascii())
    }
}

/// Macro to create constant AckCodes.
#[macro_export]
macro_rules! ack {
    ($name:ident) => {
        AckCode::from_u64($crate::protocol::constants::$name as u64)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::constants::*;

    #[test]
    fn test_4byte_ack() {
        let ack = AckCode::from_u32(BULK_ACK_DFRM);
        assert_eq!(ack.len(), 4);
        assert_eq!(ack.as_ascii(), "DFRM");
        assert!(ack.matches_u32(BULK_ACK_DFRM));
    }

    #[test]
    fn test_5byte_ack() {
        let ack = AckCode::from_u64(BULK_ACK_READY_UPH_SIZE);
        assert_eq!(ack.len(), 5);
        assert_eq!(ack.as_ascii(), "RUPHS");
    }

    #[test]
    fn test_from_bytes() {
        let bytes = b"DONE";
        let ack = AckCode::from_bytes(bytes);
        assert!(ack.matches_u32(BULK_ACK_DONE));
    }

    #[test]
    fn test_error_detection() {
        let ack = AckCode::from_u32(BULK_ACK_ER01);
        assert!(ack.is_error());
        assert_eq!(ack.as_ascii(), "ER01");
    }
}
