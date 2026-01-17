//! Application state and logic.
//!
//! Contains the app state (Model), input handling (Controller).

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dnx_core::events::{DnxEvent, DnxObserver, DnxPhase, LogLevel, PacketDirection};
use dnx_core::firmware::FirmwareAnalysis;
use dnx_core::session::{DnxSession, SessionConfig};

/// Maximum log entries to keep.
const MAX_LOG_ENTRIES: usize = 1000;

/// Application state.
pub struct App {
    /// Whether to quit the application.
    pub should_quit: bool,
    /// Current focus (which pane is active).
    pub focus: Focus,
    /// Current view/tab.
    pub current_tab: Tab,
    /// Session configuration.
    pub config: SessionConfig,
    /// Current DnX phase.
    pub phase: DnxPhase,
    /// Progress (0-100).
    pub progress: u8,
    /// Current operation name.
    pub operation: String,
    /// Log entries.
    pub logs: VecDeque<LogEntry>,
    /// Log scroll position.
    pub log_scroll: usize,
    /// Device status.
    pub device_status: DeviceStatus,
    /// File paths input.
    pub fw_dnx_path: String,
    pub fw_image_path: String,
    pub os_dnx_path: String,
    pub os_image_path: String,
    /// Input field focus.
    pub input_focus: usize,
    /// Is operation running?
    pub is_running: bool,
    /// Shared observer for receiving events from DnX session.
    pub observer: Arc<TuiObserver>,
    /// Background session thread handle.
    session_thread: Option<JoinHandle<()>>,
    /// Firmware analysis info (cached)
    pub fw_analysis: Option<FirmwareAnalysis>,
    /// Recent packets
    pub packets: VecDeque<PacketInfo>,
    /// Packet scroll position
    pub packet_scroll: usize,
}

/// Which pane is focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Config,
    Logs,
    Status,
}

/// Tab/view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Main,
    Logs,
    Protocol,
    Help,
}

/// Device connection status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceStatus {
    Disconnected,
    Connected { vid: u16, pid: u16 },
}

/// Log entry.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub timestamp: String,
}

/// Packet info for display.
#[derive(Debug, Clone)]
pub struct PacketInfo {
    pub direction: PacketDirection,
    pub timestamp: String,
    pub packet_type: String,
    pub length: usize,
    pub data_preview: String,
}

/// TUI observer that collects events for display.
pub struct TuiObserver {
    events: Mutex<VecDeque<DnxEvent>>,
}

impl TuiObserver {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(VecDeque::with_capacity(100)),
        }
    }

    pub fn drain_events(&self) -> Vec<DnxEvent> {
        let mut events = self.events.lock().unwrap();
        events.drain(..).collect()
    }
}

impl Default for TuiObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl DnxObserver for TuiObserver {
    fn on_event(&self, event: &DnxEvent) {
        let mut events = self.events.lock().unwrap();
        if events.len() >= 100 {
            events.pop_front();
        }
        events.push_back(event.clone());
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            focus: Focus::Config,
            current_tab: Tab::Main,
            config: SessionConfig::default(),
            phase: DnxPhase::WaitingForDevice,
            progress: 0,
            operation: String::new(),
            logs: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            log_scroll: 0,
            device_status: DeviceStatus::Disconnected,
            fw_dnx_path: String::new(),
            fw_image_path: String::new(),
            os_dnx_path: String::new(),
            os_image_path: String::new(),
            input_focus: 0,
            is_running: false,
            observer: Arc::new(TuiObserver::new()),
            session_thread: None,
            fw_analysis: None,
            packets: VecDeque::with_capacity(100),
            packet_scroll: 0,
        }
    }

    /// Analyze firmware file and cache result
    pub fn analyze_firmware(&mut self) {
        if !self.fw_dnx_path.is_empty() {
            let path = Path::new(&self.fw_dnx_path);
            if path.exists() {
                match FirmwareAnalysis::analyze(path) {
                    Ok(analysis) => {
                        self.add_log(
                            LogLevel::Info,
                            format!("Firmware analyzed: {}", analysis.filename),
                        );
                        self.fw_analysis = Some(analysis);
                    }
                    Err(e) => {
                        self.add_log(LogLevel::Warn, format!("Failed to analyze firmware: {}", e));
                        self.fw_analysis = None;
                    }
                }
            }
        }
    }

    /// Handle keyboard input. Returns true if app should quit.
    pub fn on_key(&mut self, key: KeyEvent) -> bool {
        // Global shortcuts
        match key.code {
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return true;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return true;
            }
            KeyCode::Esc => {
                if self.current_tab != Tab::Main {
                    self.current_tab = Tab::Main;
                    return false;
                }
                self.should_quit = true;
                return true;
            }
            KeyCode::F(1) => {
                self.current_tab = Tab::Help;
                return false;
            }
            KeyCode::F(2) => {
                self.current_tab = Tab::Logs;
                return false;
            }
            KeyCode::F(3) => {
                self.current_tab = Tab::Protocol;
                return false;
            }
            _ => {}
        }

        // Tab-specific handling
        match self.current_tab {
            Tab::Main => self.handle_main_key(key),
            Tab::Logs => self.handle_logs_key(key),
            Tab::Protocol => self.handle_protocol_key(key),
            Tab::Help => {
                // Any key returns to main
                self.current_tab = Tab::Main;
            }
        }

        false
    }

    fn handle_main_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Config => Focus::Logs,
                    Focus::Logs => Focus::Status,
                    Focus::Status => Focus::Config,
                };
            }
            KeyCode::Up => {
                if self.focus == Focus::Config && self.input_focus > 0 {
                    self.input_focus -= 1;
                }
            }
            KeyCode::Down => {
                if self.focus == Focus::Config && self.input_focus < 3 {
                    self.input_focus += 1;
                }
            }
            KeyCode::Enter => {
                if self.focus == Focus::Config && !self.is_running {
                    self.start_operation();
                }
            }
            KeyCode::Char(c) => {
                if self.focus == Focus::Config {
                    self.input_char(c);
                }
            }
            KeyCode::Backspace => {
                if self.focus == Focus::Config {
                    self.delete_char();
                }
            }
            _ => {}
        }
    }

    fn handle_logs_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.log_scroll = self.log_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.log_scroll < self.logs.len().saturating_sub(1) {
                    self.log_scroll += 1;
                }
            }
            KeyCode::PageUp => {
                self.log_scroll = self.log_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.log_scroll = (self.log_scroll + 10).min(self.logs.len().saturating_sub(1));
            }
            KeyCode::Home => {
                self.log_scroll = 0;
            }
            KeyCode::End => {
                self.log_scroll = self.logs.len().saturating_sub(1);
            }
            _ => {}
        }
    }

    fn handle_protocol_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.packet_scroll = self.packet_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.packet_scroll < self.packets.len().saturating_sub(1) {
                    self.packet_scroll += 1;
                }
            }
            KeyCode::PageUp => {
                self.packet_scroll = self.packet_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.packet_scroll =
                    (self.packet_scroll + 10).min(self.packets.len().saturating_sub(1));
            }
            KeyCode::Home => {
                self.packet_scroll = 0;
            }
            KeyCode::End => {
                self.packet_scroll = self.packets.len().saturating_sub(1);
            }
            _ => {}
        }
    }

    fn input_char(&mut self, c: char) {
        let is_fw_dnx = self.input_focus == 0;
        let field = match self.input_focus {
            0 => &mut self.fw_dnx_path,
            1 => &mut self.fw_image_path,
            2 => &mut self.os_dnx_path,
            3 => &mut self.os_image_path,
            _ => return,
        };
        field.push(c);

        // Auto-analyze when FW DnX path changes
        if is_fw_dnx {
            self.analyze_firmware();
        }
    }

    fn delete_char(&mut self) {
        let is_fw_dnx = self.input_focus == 0;
        let field = match self.input_focus {
            0 => &mut self.fw_dnx_path,
            1 => &mut self.fw_image_path,
            2 => &mut self.os_dnx_path,
            3 => &mut self.os_image_path,
            _ => return,
        };
        field.pop();

        // Auto-analyze when FW DnX path changes
        if is_fw_dnx {
            self.analyze_firmware();
        }
    }

    fn start_operation(&mut self) {
        if self.is_running {
            return;
        }

        if self.fw_dnx_path.is_empty()
            && self.fw_image_path.is_empty()
            && self.os_dnx_path.is_empty()
            && self.os_image_path.is_empty()
        {
            self.add_log(
                LogLevel::Error,
                "No files selected! Please enter file paths.",
            );
            return;
        }

        self.is_running = true;
        self.phase = DnxPhase::WaitingForDevice;
        self.progress = 0;
        self.operation = "Starting...".to_string();

        // Build config from UI fields using the unified API
        let session_config = SessionConfig::default()
            .merge(
                if self.fw_dnx_path.is_empty() {
                    None
                } else {
                    Some(self.fw_dnx_path.clone())
                },
                if self.fw_image_path.is_empty() {
                    None
                } else {
                    Some(self.fw_image_path.clone())
                },
                if self.os_dnx_path.is_empty() {
                    None
                } else {
                    Some(self.os_dnx_path.clone())
                },
                if self.os_image_path.is_empty() {
                    None
                } else {
                    Some(self.os_image_path.clone())
                },
                None, // misc_dnx
                Some(self.config.gp_flags),
                Some(self.config.ifwi_wipe_enable),
            )
            .with_defaults();

        self.add_log(LogLevel::Info, "Operation started");

        // Clone observer for the thread
        let observer = self.observer.clone();

        // Spawn session thread
        let handle = thread::spawn(move || {
            let mut session = DnxSession::with_observer(session_config, observer.clone());
            match session.run() {
                Ok(_) => {
                    observer.on_event(&DnxEvent::Complete);
                }
                Err(e) => {
                    observer.on_event(&DnxEvent::Error {
                        code: 1, // Generic error code
                        message: format!("Session error: {}", e),
                    });
                }
            }
        });

        self.session_thread = Some(handle);
    }

    /// Called on each tick - process observer events.
    pub fn on_tick(&mut self) {
        // Process events from observer
        let events = self.observer.drain_events();
        for event in events {
            self.process_dnx_event(event);
        }
    }

    fn process_dnx_event(&mut self, event: DnxEvent) {
        match event {
            DnxEvent::DeviceConnected { vid, pid } => {
                self.device_status = DeviceStatus::Connected { vid, pid };
                self.add_log(
                    LogLevel::Info,
                    format!("Device connected: {:04X}:{:04X}", vid, pid),
                );
            }
            DnxEvent::DeviceDisconnected => {
                self.device_status = DeviceStatus::Disconnected;
                self.add_log(LogLevel::Warn, "Device disconnected");
            }
            DnxEvent::PhaseChanged { to, .. } => {
                self.phase = to;
                self.add_log(LogLevel::Info, format!("Phase: {}", to));
            }
            DnxEvent::Progress {
                operation,
                current,
                total,
                ..
            } => {
                self.operation = operation;
                self.progress = if total > 0 {
                    ((current * 100) / total) as u8
                } else {
                    0
                };
            }
            DnxEvent::Log { level, message } => {
                self.add_log(level, message);
            }
            DnxEvent::AckReceived { ack } => {
                self.add_log(LogLevel::Debug, format!("ACK: {}", ack));
            }
            DnxEvent::Error { message, .. } => {
                self.add_log(LogLevel::Error, message);
                self.is_running = false;
            }
            DnxEvent::Complete => {
                self.is_running = false;
                self.progress = 100;
                self.add_log(LogLevel::Info, "Operation complete!");
            }
            DnxEvent::Packet {
                direction,
                packet_type,
                length,
                data,
            } => {
                let now = chrono::Local::now();
                let data_preview = if let Some(d) = data {
                    d.iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    String::new()
                };

                let packet = PacketInfo {
                    direction,
                    timestamp: now.format("%H:%M:%S.%3f").to_string(), // Milliseconds
                    packet_type,
                    length,
                    data_preview,
                };

                if self.packets.len() >= 1000 {
                    self.packets.pop_front();
                }
                self.packets.push_back(packet);
                // Auto-scroll
                self.packet_scroll = self.packets.len().saturating_sub(1);
            }
        }
    }

    fn add_log(&mut self, level: LogLevel, message: impl Into<String>) {
        let now = chrono::Local::now();
        let entry = LogEntry {
            level,
            message: message.into(),
            timestamp: now.format("%H:%M:%S").to_string(),
        };

        if self.logs.len() >= MAX_LOG_ENTRIES {
            self.logs.pop_front();
        }
        self.logs.push_back(entry);

        // Auto-scroll to bottom
        self.log_scroll = self.logs.len().saturating_sub(1);
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
