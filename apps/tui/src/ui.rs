//! UI rendering module.
//!
//! Contains all the widget rendering logic (View).

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Gauge, List, ListItem, Padding, Paragraph, Tabs, Wrap},
};

use crate::app::{App, DeviceStatus, Focus, LogEntry, Tab};
use dnx_core::events::{DnxPhase, LogLevel};

/// Main draw function.
pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Create main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header/tabs
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Footer/status bar
        ])
        .split(area);

    draw_header(frame, chunks[0], app);

    match app.current_tab {
        Tab::Main => draw_main_view(frame, chunks[1], app),
        Tab::Logs => draw_logs_view(frame, chunks[1], app),
        Tab::Help => draw_help_view(frame, chunks[1]),
    }

    draw_footer(frame, chunks[2], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let titles = vec!["Main", "Logs (F2)", "Help (F1)"];
    let selected = match app.current_tab {
        Tab::Main => 0,
        Tab::Logs => 1,
        Tab::Help => 2,
    };

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" DnX-RS TUI ")
                .title_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .select(selected)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .divider(symbols::DOT);

    frame.render_widget(tabs, area);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let status = match &app.device_status {
        DeviceStatus::Disconnected => {
            Span::styled(" ○ Disconnected ", Style::default().fg(Color::Red))
        }
        DeviceStatus::Connected { vid, pid } => Span::styled(
            format!(" ● {:04X}:{:04X} ", vid, pid),
            Style::default().fg(Color::Green),
        ),
        DeviceStatus::Busy => Span::styled(" ◐ Busy ", Style::default().fg(Color::Yellow)),
    };

    let phase = Span::styled(format!(" {} ", app.phase), Style::default().fg(Color::Cyan));

    let help = Span::styled(
        " Ctrl+Q: Quit | Tab: Focus | Enter: Start ",
        Style::default().fg(Color::DarkGray),
    );

    let line = Line::from(vec![status, phase, help]);

    let footer = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(footer, area);
}

fn draw_main_view(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Config
            Constraint::Percentage(60), // Status & Progress
        ])
        .split(area);

    draw_config_panel(frame, chunks[0], app);
    draw_status_panel(frame, chunks[1], app);
}

fn draw_config_panel(frame: &mut Frame, area: Rect, app: &App) {
    let is_focused = app.focus == Focus::Config;
    let border_color = if is_focused {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(" Configuration ")
        .title_style(Style::default().fg(if is_focused {
            Color::Yellow
        } else {
            Color::White
        }))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Input fields
    let fields = [
        ("FW DnX:  ", &app.fw_dnx_path),
        ("FW Image:", &app.fw_image_path),
        ("OS DnX:  ", &app.os_dnx_path),
        ("OS Image:", &app.os_image_path),
    ];

    let field_height = 3;
    let fields_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            fields
                .iter()
                .map(|_| Constraint::Length(field_height))
                .chain(std::iter::once(Constraint::Min(1))) // Start button
                .collect::<Vec<_>>(),
        )
        .split(inner);

    for (i, (label, value)) in fields.iter().enumerate() {
        let is_active = is_focused && app.input_focus == i;
        let style = if is_active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };

        let display_value = if value.is_empty() {
            "(not set)".to_string()
        } else {
            value.to_string()
        };

        let cursor = if is_active { "▏" } else { "" };

        let input =
            Paragraph::new(Line::from(vec![
                Span::styled(*label, Style::default().fg(Color::Cyan)),
                Span::styled(&display_value, style),
                Span::styled(cursor, Style::default().fg(Color::Yellow)),
            ]))
            .block(Block::default().borders(Borders::BOTTOM).border_style(
                if is_active {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ));

        frame.render_widget(input, fields_layout[i]);
    }

    // Start button
    let button_style = if app.is_running {
        Style::default().fg(Color::DarkGray)
    } else if is_focused && app.input_focus > 3 {
        Style::default().fg(Color::Black).bg(Color::Yellow)
    } else {
        Style::default().fg(Color::Green)
    };

    let button_text = if app.is_running {
        "⟳ Running..."
    } else {
        "▶ Start"
    };
    let button = Paragraph::new(button_text)
        .style(button_style)
        .alignment(ratatui::layout::Alignment::Center);

    if fields_layout.len() > 4 {
        frame.render_widget(button, fields_layout[4]);
    }
}

fn draw_status_panel(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Progress
            Constraint::Min(5),    // Recent logs
        ])
        .split(area);

    // Progress gauge
    draw_progress(frame, chunks[0], app);

    // Recent logs
    draw_recent_logs(frame, chunks[1], app);
}

fn draw_progress(frame: &mut Frame, area: Rect, app: &App) {
    let color = match app.phase {
        DnxPhase::Complete => Color::Green,
        DnxPhase::Error => Color::Red,
        _ => Color::Cyan,
    };

    let label = if app.operation.is_empty() {
        format!("{}%", app.progress)
    } else {
        format!("{}: {}%", app.operation, app.progress)
    };

    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" Progress "),
        )
        .gauge_style(Style::default().fg(color).bg(Color::Black))
        .percent(app.progress as u16)
        .label(label);

    frame.render_widget(gauge, area);
}

fn draw_recent_logs(frame: &mut Frame, area: Rect, app: &App) {
    let is_focused = app.focus == Focus::Logs;
    let border_color = if is_focused {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let items: Vec<ListItem> = app
        .logs
        .iter()
        .rev()
        .take(area.height.saturating_sub(2) as usize)
        .map(log_to_list_item)
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(" Recent Logs "),
        )
        .style(Style::default().fg(Color::White));

    frame.render_widget(list, area);
}

fn draw_logs_view(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .logs
        .iter()
        .skip(app.log_scroll)
        .take(area.height.saturating_sub(2) as usize)
        .map(log_to_list_item)
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(format!(
                    " Logs ({}/{}) ",
                    app.log_scroll + 1,
                    app.logs.len().max(1)
                )),
        )
        .style(Style::default().fg(Color::White));

    frame.render_widget(list, area);
}

fn draw_help_view(frame: &mut Frame, area: Rect) {
    let help_text = vec![
        "",
        "  DnX-RS TUI - Intel DnX Protocol Tool",
        "",
        "  KEYBOARD SHORTCUTS:",
        "",
        "  Ctrl+Q, Ctrl+C, Esc    Quit application",
        "  F1                     Show this help",
        "  F2                     View full logs",
        "  Tab                    Switch focus between panels",
        "  Up/Down                Navigate input fields",
        "  Enter                  Start DnX operation",
        "",
        "  IN LOGS VIEW:",
        "",
        "  j/k, Up/Down           Scroll logs",
        "  Page Up/Down           Scroll by page",
        "  Home/End               Go to start/end",
        "",
        "  USAGE:",
        "",
        "  1. Fill in the file paths (use Tab/Arrow keys)",
        "  2. Press Enter to start the operation",
        "  3. Watch the progress and logs",
        "",
        "  Press any key to return...",
    ];

    let text: Vec<Line> = help_text.iter().map(|s| Line::from(*s)).collect();

    let help = Paragraph::new(Text::from(text))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Help "),
        )
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false });

    frame.render_widget(help, area);
}

fn log_to_list_item(entry: &LogEntry) -> ListItem<'static> {
    let (icon, color) = match entry.level {
        LogLevel::Error => ("✗", Color::Red),
        LogLevel::Warn => ("⚠", Color::Yellow),
        LogLevel::Info => ("●", Color::Green),
        LogLevel::Debug => ("○", Color::Blue),
        LogLevel::Trace => ("·", Color::DarkGray),
    };

    let line = Line::from(vec![
        Span::styled(
            format!("{} ", entry.timestamp),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(format!("{} ", icon), Style::default().fg(color)),
        Span::styled(entry.message.clone(), Style::default().fg(Color::White)),
    ]);

    ListItem::new(line)
}
