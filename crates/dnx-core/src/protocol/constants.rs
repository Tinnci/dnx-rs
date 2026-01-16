//! Protocol constants from Intel Medfield DnX specification.
//!
//! Derived from xFSTK `medfieldmessages.h`.

// ============================================================================
// Device Identification
// ============================================================================

/// Intel Corporation Vendor ID
pub const INTEL_VENDOR_ID: u16 = 0x8086;

// Medfield Platform PIDs
/// Medfield platform Product ID (ROM stage)
pub const MEDFIELD_PRODUCT_ID: u16 = 0xE004;
/// Medfield FW stage PID
pub const MEDFIELD_FW_PID: u16 = 0x0A14;

// Moorefield Platform PIDs (Atom Z3560/Z3580)
/// Moorefield platform Product ID (DnX mode)
pub const MOOREFIELD_PRODUCT_ID: u16 = 0x0A2C;
/// Moorefield alternative PID
pub const MOOREFIELD_ALT_PID: u16 = 0x0A65;

/// All supported PIDs for device discovery
pub const SUPPORTED_PIDS: &[u16] = &[
    MEDFIELD_PRODUCT_ID,
    MEDFIELD_FW_PID,
    MOOREFIELD_PRODUCT_ID,
    MOOREFIELD_ALT_PID,
];

// ============================================================================
// Size Constants
// ============================================================================

pub const MAX_PKT_SIZE: usize = 0x200; // 512 bytes
pub const TWO_K: usize = 1024 * 2;
pub const ONE28_K: usize = 1024 * 128; // 128 KB chunk size
pub const NINETY_SIX_KB: usize = 1024 * 96;
pub const TWO_HUNDRED_KB: usize = 1024 * 200;
pub const TWO_MB: usize = 2 * 1024 * 1024;

/// DnX FW Header Size (6 DWORDs = 24 bytes)
pub const DNX_FW_SIZE_HDR_SIZE: usize = 0x18;

/// FW Update Profile Header sizes (platform-dependent)
pub const D0_FW_UPDATE_PROFILE_HDR_SIZE: usize = 0x24;
pub const C0_FW_UPDATE_PROFILE_HDR_SIZE: usize = 0x20;
pub const FW_UPDATE_PROFILE_OLD_HDR_SIZE_MFD: usize = 0x1C;

/// OSIP Partition Table Size
pub const OSIP_PARTITIONTABLE_SIZE: usize = 0x200;

// ============================================================================
// Preambles (Host -> Device)
// ============================================================================

/// Download Execute ROM - Initial handshake
pub const PREAMBLE_DNER: u32 = 0x52456E44; // 'DnER'

/// ID Request
pub const PREAMBLE_IDRQ: u32 = 0x51524449; // 'IDRQ'

/// Boot Mode Request
pub const PREAMBLE_BMRQ: u32 = 0x51524D42; // 'BMRQ'

// ============================================================================
// Firmware Upgrade ACK Codes (Device -> Host)
// ============================================================================

/// Virgin part DnX (no existing firmware)
pub const BULK_ACK_DFRM: u32 = 0x4446524D; // 'DFRM'

/// Non-virgin part DnX (existing firmware)
#[allow(non_upper_case_globals)]
pub const BULK_ACK_DxxM: u32 = 0x4478784D; // 'DxxM'

/// Download Execute Bootloader - Ready for DnX binary
pub const BULK_ACK_DXBL: u32 = 0x4458424C; // 'DXBL'

/// Ready for Update Profile Header Size (5 bytes)
pub const BULK_ACK_READY_UPH_SIZE: u64 = 0x5255504853; // 'RUPHS'

/// Ready for Update Profile Header
pub const BULK_ACK_READY_UPH: u32 = 0x52555048; // 'RUPH'

/// GPP Reset signal (5 bytes)
pub const BULK_ACK_GPP_RESET: u64 = 0x5245534554; // 'RESET'

/// Download MIP (Module Info Pointer)
pub const BULK_ACK_DMIP: u32 = 0x444D4950; // 'DMIP'

/// Low FW - First 128KB chunk
pub const BULK_ACK_LOFW: u32 = 0x4C4F4657; // 'LOFW'

/// High FW - Second 128KB chunk
pub const BULK_ACK_HIFW: u32 = 0x48494657; // 'HIFW'

/// Primary Security FW 1 (5 bytes)
pub const BULK_ACK_PSFW1: u64 = 0x5053465731; // 'PSFW1'

/// Primary Security FW 2 (5 bytes)
pub const BULK_ACK_PSFW2: u64 = 0x5053465732; // 'PSFW2'

/// Secondary Security FW
pub const BULK_ACK_SSFW: u32 = 0x53534657; // 'SSFW'

/// Update Successful
pub const BULK_ACK_UPDATE_SUCCESSFUL: u32 = 0x484C5424; // 'HLT$'

/// Medfield platform identifier
pub const BULK_ACK_MFLD: u32 = 0x4D464C44; // 'MFLD'

/// Clovertrail platform identifier
pub const BULK_ACK_CLVT: u32 = 0x434C5654; // 'CLVT'

/// ROM Patch / Security uCode Patch
pub const BULK_ACK_PATCH: u32 = 0x53754350; // 'SuCP'

/// RTBD (unknown purpose)
pub const BULK_ACK_RTBD: u32 = 0x52544244; // 'RTBD'

/// Video Encoder/Decoder FW (5 bytes)
pub const BULK_ACK_VEDFW: u64 = 0x5645444657; // 'VEDFW'

/// Secondary Security BIOS
pub const BULK_ACK_SSBS: u32 = 0x53534253; // 'SSBS'

/// IFWI partitions (IFW1, IFW2, IFW3)
pub const BULK_ACK_IFW1: u32 = 0x49465701; // 'IFW\x01'
pub const BULK_ACK_IFW2: u32 = 0x49465702; // 'IFW\x02'
pub const BULK_ACK_IFW3: u32 = 0x49465703; // 'IFW\x03'

/// HLT0 - FW file has no size
pub const BULK_ACK_HLT0: u32 = 0x484C5430; // 'HLT0'

// ============================================================================
// OS Recovery ACK Codes
// ============================================================================

/// OS Recovery Mode
pub const BULK_ACK_DORM: u32 = 0x444F524D; // 'DORM'

/// OSIP Size request (7 bytes - unusual!)
pub const BULK_ACK_OSIPSZ: u64 = 0x4F53495020537A; // 'OSIP Sz'

/// Ready for OSIP (5 bytes)
pub const BULK_ACK_ROSIP: u64 = 0x524F534950; // 'ROSIP'

/// Done
pub const BULK_ACK_DONE: u32 = 0x444F4E45; // 'DONE'

/// Request Image chunk
pub const BULK_ACK_RIMG: u32 = 0x52494D47; // 'RIMG'

/// End of Image Update
pub const BULK_ACK_EOIU: u32 = 0x454F4955; // 'EOIU'

// ============================================================================
// Error Codes
// ============================================================================

pub const BULK_ACK_INVALID_PING: u32 = 0x45523030; // 'ER00'
pub const BULK_ACK_ER01: u32 = 0x45523031; // 'ER01'
pub const BULK_ACK_ER02: u32 = 0x45523032; // 'ER02'
pub const BULK_ACK_ER03: u32 = 0x45523033; // 'ER03'
pub const BULK_ACK_ER04: u32 = 0x45523034; // 'ER04'
pub const BULK_ACK_ER10: u32 = 0x45523130; // 'ER10'
pub const BULK_ACK_ER11: u32 = 0x45523131; // 'ER11'
pub const BULK_ACK_ER12: u32 = 0x45523132; // 'ER12'
pub const BULK_ACK_ER13: u32 = 0x45523133; // 'ER13'
pub const BULK_ACK_ER15: u32 = 0x45523135; // 'ER15'
pub const BULK_ACK_ER16: u32 = 0x45523136; // 'ER16'
pub const BULK_ACK_ER17: u32 = 0x45523137; // 'ER17'
pub const BULK_ACK_ER18: u32 = 0x45523138; // 'ER18'
pub const BULK_ACK_ER20: u32 = 0x45523230; // 'ER20'
pub const BULK_ACK_ER21: u32 = 0x45523231; // 'ER21'
pub const BULK_ACK_ER22: u32 = 0x45523232; // 'ER22'
pub const BULK_ACK_ER25: u32 = 0x45523235; // 'ER25'
pub const BULK_ACK_ERRR: u32 = 0x45525252; // 'ERRR'

// ============================================================================
// Operation Codes
// ============================================================================

pub const OPP_FW: u8 = 0;
pub const OPP_OS: u8 = 1;

/// DFRM operation code
pub const DFRM_OPCODE: u16 = 0x1000;

/// DxxM operation code
#[allow(non_upper_case_globals)]
pub const DxxM_OPCODE: u16 = 0x2000;

/// DORM operation code
pub const DORM_OPCODE: u16 = 0x3000;

// ============================================================================
// Size Offsets in FW Update Profile Header
// ============================================================================

pub const PSFW1_SIZE_OFFSET: usize = 0x0C;
pub const PSFW2_SIZE_OFFSET: usize = 0x10;
pub const SSFW_SIZE_OFFSET: usize = 0x14;
pub const ROM_PATCH_SIZE_OFFSET: usize = 0x18;

// ============================================================================
// OSIP Offsets
// ============================================================================

pub const OSIP_SIZE_OFFSET: usize = 0x04;
pub const OSIP_NUM_POINTERS_OFFSET: usize = 0x08;

/// Calculate offset for OS partition N size
#[inline]
pub const fn get_os_n_size_offset(n: usize) -> usize {
    (n * 0x18) + 0x30
}
