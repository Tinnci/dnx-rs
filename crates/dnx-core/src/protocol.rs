pub const MAX_ERR_MSG: usize = 200;
pub const MFDLIB_VERSION: u16 = 0x0107;
pub const MAX_DEVPATH_LENGTH: usize = 256;
pub const MAX_PIPES: usize = 16;
pub const MAX_DATA_LEN: usize = 3996;
pub const RETRY_ATTEMPTS: u8 = 2;
pub const MAX_PKT_SIZE: usize = 0x200;
pub const TWO_K: usize = 1024 * 2;
pub const ONE28_K: usize = 1024 * 128;
pub const NINETY_SIX_KB: usize = 1024 * 96;
pub const ONE40_KB: usize = 1024 * 140;
pub const TWO_HUNDRED_KB: usize = 1024 * 200;
pub const TWO_MB: usize = 2 * 1024 * 1024;
pub const EIGHT_SIXTY_KB_PLUS_VRL: usize = 0xD71E0;
pub const FW_USB_IMAGE_SIZE: usize = 0x40800;
pub const DNX_FW_SIZE_HDR_SIZE: usize = 0x18;
pub const D0_FW_UPDATE_PROFILE_HDR_SIZE: usize = 0x24;
pub const C0_FW_UPDATE_PROFILE_HDR_SIZE: usize = 0x20;
pub const FW_UPDATE_PROFILE_OLD_HDR_SIZE_MFD: usize = 0x1C;
pub const PSFW1_SIZE_OFFSET: usize = 0x0C;
pub const PSFW2_SIZE_OFFSET: usize = 0x10;
pub const SSFW_SIZE_OFFSET: usize = 0x14;
pub const ROM_PATCH_SIZE_OFFSET: usize = 0x18;

pub const PREAMBLE_RETRY_TIMEOUT: u64 = 86400000;

// opp codes
pub const OPP_FW: u8 = 0;
pub const OPP_OS: u8 = 1;

// Serial Start
pub const SERIAL_START: u32 = 0x536F5478; // 'SoTx'

// FW/OS Preambles
pub const PREAMBLE_DNER: u32 = 0x52456E44; // 'DnER'
pub const PREAMBLE_IDRQ: u32 = 0x51524449; // 'IDRQ'
pub const PREAMBLE_BMRQ: u32 = 0x51524D42; // 'BMRQ'

// BATI battery status preambles
pub const PREAMBLE_DBDS: u32 = 0x53444244; // 'DBDS'
pub const PREAMBLE_RRBD: u32 = 0x44425252; // 'RRBD'
pub const BATI_SIGNATURE: u32 = 0x42415449; // 'BATI'

// FW Upgrade Ack values
pub const BULK_ACK_DFRM: u32 = 0x4446524D; // 'DFRM'
pub const BULK_ACK_DXM: u32 = 0x4478784D; // 'DxxM'
pub const BULK_ACK_DXBL: u32 = 0x4458424C; // 'DXBL'
pub const BULK_ACK_READY_UPH_SIZE: u64 = 0x5255504853; // 'RUPHS' (Wait, this is 5 bytes? 0x52 55 50 48 53. The u32/u64 definitions in original C code used ULL, but let's check carefully. 'RUPHS' is 5 chars. 0x5255504853.
pub const BULK_ACK_READY_UPH: u32 = 0x52555048; // 'RUPH'
pub const BULK_ACK_GPP_RESET: u64 = 0x5245534554; // 'RESET' (5 bytes)
pub const BULK_ACK_DMIP: u32 = 0x444D4950; // 'DMIP'
pub const BULK_ACK_LOFW: u32 = 0x4C4F4657; // 'LOFW'
pub const BULK_ACK_HIFW: u32 = 0x48494657; // 'HIFW'
pub const BULK_ACK_PSFW1: u64 = 0x5053465731; // 'PSFW1'
pub const BULK_ACK_PSFW2: u64 = 0x5053465732; // 'PSFW2'
pub const BULK_ACK_SSFW: u32 = 0x53534657; // 'SSFW'
pub const BULK_ACK_UPDATE_SUCESSFUL: u32 = 0x484C5424; // 'HLT$'
pub const BULK_ACK_MFLD: u32 = 0x4D464C44; // 'MFLD'
pub const BULK_ACK_CLVT: u32 = 0x434C5654; // 'CLVT'
pub const BULK_ACK_PATCH: u32 = 0x53754350; // 'SuCP'
pub const BULK_ACK_RTBD: u32 = 0x52544244; // 'RTBD'
pub const BULK_ACK_VEDFW: u64 = 0x5645444657; // 'VEDFW'
pub const BULK_ACK_SSBS: u32 = 0x53534253; // 'SSBS'
pub const BULK_ACK_IFW1: u32 = 0x49465701; // 'IFW1'
pub const BULK_ACK_IFW2: u32 = 0x49465702; // 'IFW2'
pub const BULK_ACK_IFW3: u32 = 0x49465703; // 'IFW3'

pub const BULK_ACK_INVALID_PING: u32 = 0x45523030; // 'ER00'
pub const BULK_ACK_HLT0: u32 = 0x484C5430; // 'HLT0'
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
pub const BULK_ACK_ERB0: u32 = 0x45524230; // 'ERB0'
pub const BULK_ACK_ERB1: u32 = 0x45524231; // 'ERB1'

// OS Recovery Ack values
pub const BULK_ACK_DORM: u32 = 0x444F524D; // 'DORM'
pub const BULK_ACK_OSIPSZ: u64 = 0x4F53495020537A; // 'OSIP Sz'
pub const BULK_ACK_ROSIP: u64 = 0x524F534950; // 'ROSIP' (Wait, this is 5 bytes: 52 4F 53 49 50)
pub const BULK_ACK_DONE: u32 = 0x444F4E45; // 'DONE'
pub const BULK_ACK_RIMG: u32 = 0x52494D47; // 'RIMG'
pub const BULK_ACK_EOIU: u32 = 0x454F4955; // 'EOIU'

pub const INTEL_VENDOR_ID: u16 = 0x8086;
pub const MEDFIELD_PRODUCT_ID: u16 = 0xE004;
