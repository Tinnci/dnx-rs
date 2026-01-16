//! DnX TUI Application - Terminal User Interface
//!
//! A full-featured TUI for Intel DnX protocol operations with real-time
//! progress display, log viewer, and status indicators.

mod app;
mod event;
mod ui;

use std::io;
use std::panic;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::*;
use tracing_subscriber::prelude::*;

use app::App;
use event::{Event, EventHandler};

fn main() -> Result<()> {
    // Setup panic hook to restore terminal on crash
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        original_hook(panic_info);
    }));

    // Initialize tracing to file (not stdout, since we're using the terminal)
    let file_appender = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(|| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("dnx-tui.log")
                .unwrap()
        });

    tracing_subscriber::registry()
        .with(file_appender)
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Setup terminal
    let terminal = setup_terminal()?;

    // Run app
    let result = run_app(terminal);

    // Restore terminal
    restore_terminal()?;

    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

fn run_app(mut terminal: Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new();
    let event_handler = EventHandler::new(250); // 250ms tick rate

    loop {
        // Draw UI
        terminal.draw(|frame| ui::draw(frame, &app))?;

        // Handle events
        match event_handler.next()? {
            Event::Tick => {
                app.on_tick();
            }
            Event::Key(key_event) => {
                if app.on_key(key_event) {
                    break; // Exit requested
                }
            }
            Event::Mouse(_) => {
                // Mouse events not handled yet
            }
            Event::Resize(_, _) => {
                // Terminal resize is handled automatically by ratatui
            }
        }
    }

    Ok(())
}
