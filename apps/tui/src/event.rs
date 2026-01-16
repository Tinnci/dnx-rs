//! Event handling for the TUI.
//!
//! Provides an event loop that captures keyboard, mouse, and terminal events.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, KeyEvent, MouseEvent};

/// Terminal events.
#[derive(Debug, Clone)]
pub enum Event {
    /// Periodic tick for animations/updates.
    Tick,
    /// Keyboard event.
    Key(KeyEvent),
    /// Mouse event.
    #[allow(dead_code)]
    Mouse(MouseEvent),
    /// Terminal resize.
    #[allow(dead_code)]
    Resize(u16, u16),
}

/// Event handler that runs in a background thread.
pub struct EventHandler {
    receiver: mpsc::Receiver<Event>,
    #[allow(dead_code)]
    handler: thread::JoinHandle<()>,
}

impl EventHandler {
    /// Create a new event handler with the specified tick rate.
    pub fn new(tick_rate_ms: u64) -> Self {
        let tick_rate = Duration::from_millis(tick_rate_ms);
        let (sender, receiver) = mpsc::channel();

        let handler = thread::spawn(move || {
            let mut last_tick = std::time::Instant::now();
            loop {
                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or(Duration::ZERO);

                if event::poll(timeout).unwrap_or(false) {
                    if let Ok(evt) = event::read() {
                        let event = match evt {
                            event::Event::Key(key) => Some(Event::Key(key)),
                            event::Event::Mouse(mouse) => Some(Event::Mouse(mouse)),
                            event::Event::Resize(w, h) => Some(Event::Resize(w, h)),
                            _ => None,
                        };
                        if let Some(e) = event {
                            if sender.send(e).is_err() {
                                break;
                            }
                        }
                    }
                }

                if last_tick.elapsed() >= tick_rate {
                    if sender.send(Event::Tick).is_err() {
                        break;
                    }
                    last_tick = std::time::Instant::now();
                }
            }
        });

        Self { receiver, handler }
    }

    /// Get the next event, blocking.
    pub fn next(&self) -> Result<Event> {
        Ok(self.receiver.recv()?)
    }
}
