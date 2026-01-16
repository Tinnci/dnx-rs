//! State machine implementation for DnX protocol.

use std::fmt;

/// Internal state of the DnX downloader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DldrState {
    /// Invalid / initial state.
    Invalid,
    /// Normal firmware download (virgin part).
    FwNormal,
    /// Misc firmware download (DnX OS mode).
    FwMisc,
    /// IFWI wipe mode.
    FwWipe,
    /// Normal OS download.
    OsNormal,
    /// Misc OS download.
    OsMisc,
}

impl Default for DldrState {
    fn default() -> Self {
        Self::Invalid
    }
}

impl fmt::Display for DldrState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DldrState::Invalid => write!(f, "INVALID"),
            DldrState::FwNormal => write!(f, "FW_NORMAL"),
            DldrState::FwMisc => write!(f, "FW_MISC"),
            DldrState::FwWipe => write!(f, "FW_WIPE"),
            DldrState::OsNormal => write!(f, "OS_NORMAL"),
            DldrState::OsMisc => write!(f, "OS_MISC"),
        }
    }
}

impl DldrState {
    /// Check if this is a firmware state.
    pub fn is_fw(&self) -> bool {
        matches!(
            self,
            DldrState::FwNormal | DldrState::FwMisc | DldrState::FwWipe
        )
    }

    /// Check if this is an OS state.
    pub fn is_os(&self) -> bool {
        matches!(self, DldrState::OsNormal | DldrState::OsMisc)
    }
}

/// Firmware chunk tracking.
#[derive(Debug, Default)]
pub struct ChunkTracker {
    /// Total chunks to send.
    pub total_chunks: usize,
    /// Current chunk index.
    pub current_chunk: usize,
    /// Residual bytes after last full chunk.
    pub residual_bytes: usize,
    /// Byte offset into the data.
    pub byte_offset: usize,
}

impl ChunkTracker {
    pub fn new(data_size: usize, chunk_size: usize) -> Self {
        let total_chunks = data_size / chunk_size;
        let residual = data_size % chunk_size;
        Self {
            total_chunks: if residual == 0 && total_chunks > 0 {
                total_chunks - 1
            } else {
                total_chunks
            },
            current_chunk: 0,
            residual_bytes: residual,
            byte_offset: 0,
        }
    }

    pub fn reset(&mut self) {
        self.current_chunk = 0;
        self.byte_offset = 0;
    }

    pub fn advance(&mut self, chunk_size: usize) {
        self.current_chunk += 1;
        self.byte_offset += chunk_size;
    }

    pub fn is_done(&self) -> bool {
        self.current_chunk > self.total_chunks
    }

    pub fn is_last_chunk(&self) -> bool {
        self.current_chunk == self.total_chunks
    }
}

/// State machine context holding all runtime state.
#[derive(Debug, Default)]
pub struct StateMachineContext {
    /// Current downloader state.
    pub state: DldrState,
    /// Whether FW download is complete.
    pub fw_done: bool,
    /// Whether IFWI is done.
    pub ifwi_done: bool,
    /// Whether OS download is complete.
    pub os_done: bool,
    /// Whether operation was aborted.
    pub abort: bool,
    /// Whether GPP reset was received.
    pub gpp_reset: bool,
    /// Flags from GP (General Purpose).
    pub gp_flags: u32,
    /// IFWI wipe enabled.
    pub ifwi_wipe_enable: bool,
    /// Chunk trackers for various FW components.
    pub psfw1_tracker: ChunkTracker,
    pub psfw2_tracker: ChunkTracker,
    pub ssfw_tracker: ChunkTracker,
    pub vedfw_tracker: ChunkTracker,
    pub rom_patch_tracker: ChunkTracker,
}

impl StateMachineContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Transition to a new state.
    pub fn goto_state(&mut self, new_state: DldrState) {
        tracing::info!(from = %self.state, to = %new_state, "State transition");
        self.state = new_state;
    }

    /// Check if operation should continue.
    pub fn should_continue(&self) -> bool {
        !self.abort && !self.is_complete()
    }

    /// Check if all operations are complete.
    pub fn is_complete(&self) -> bool {
        (self.fw_done || self.gpp_reset) && self.os_done
    }
}
