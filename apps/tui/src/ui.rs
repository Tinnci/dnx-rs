//! UI rendering module.
//!
//! Contains all the widget rendering logic (View).

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
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

        let input_area_width = fields_layout[i].width as usize - 2; // -2 for borders
        let display_value = if value.len() > input_area_width {
            format!("...{}", &value[value.len() - (input_area_width - 3)..])
        } else {
            value.to_string()
        };

        let cursor = if is_active { "▏" } else { "" };

        let input =
            Paragraph::new(Line::from(vec![
                Span::styled(*label, Style::default().fg(Color::Cyan)),
                Span::styled(display_value, style),
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
            Constraint::Length(8), // Firmware info (NEW)
            Constraint::Min(5),    // Recent logs
        ])
        .split(area);

    // Progress gauge
    draw_progress(frame, chunks[0], app);

    // Firmware info panel (NEW)
    draw_firmware_info(frame, chunks[1], app);

    // Recent logs
    draw_recent_logs(frame, chunks[2], app);
}

fn draw_firmware_info(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Firmware Info ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let content = if let Some(analysis) = &app.fw_analysis {
        let type_str = format!("{}", analysis.file_type);
        let size_str = format!(
            "{} bytes ({:.1} KB)",
            analysis.size,
            analysis.size as f64 / 1024.0
        );

        let token_str = if let Some(t) = &analysis.token {
            format!("{} - {}", t.marker, t.platform)
        } else {
            "N/A".to_string()
        };

        let chaabi_str = if let Some(c) = &analysis.chaabi {
            format!("{} bytes ({:.1} KB)", c.size, c.size as f64 / 1024.0)
        } else {
            "N/A".to_string()
        };

        let rsa_str = if analysis.rsa_signature.is_some() {
            "✅ Intel Signed"
        } else {
            "❌ Not found"
        };

        let valid_str = if analysis.is_valid() {
            "✅ Valid"
        } else {
            "⚠️ Issues"
        };

        vec![
            Line::from(vec![
                Span::styled("Type: ", Style::default().fg(Color::Cyan)),
                Span::styled(type_str, Style::default().fg(Color::White)),
                Span::raw("  "),
                Span::styled("Size: ", Style::default().fg(Color::Cyan)),
                Span::styled(size_str, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("Token: ", Style::default().fg(Color::Cyan)),
                Span::styled(token_str, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("Chaabi: ", Style::default().fg(Color::Cyan)),
                Span::styled(chaabi_str, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("RSA: ", Style::default().fg(Color::Cyan)),
                Span::styled(rsa_str, Style::default().fg(Color::White)),
                Span::raw("  "),
                Span::styled("Status: ", Style::default().fg(Color::Cyan)),
                Span::styled(valid_str, Style::default().fg(Color::White)),
            ]),
        ]
    } else {
        vec![
            Line::from(Span::styled(
                "No firmware loaded",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "Enter a FW DnX path to analyze",
                Style::default().fg(Color::DarkGray),
            )),
        ]
    };

    let paragraph = Paragraph::new(content);
    frame.render_widget(paragraph, inner);
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
        .map(|entry| log_to_list_item(entry, area.width))
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
        .map(|entry| log_to_list_item(entry, area.width))
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

fn log_to_list_item(entry: &LogEntry, width: u16) -> ListItem<'static> {
    let (icon, color) = match entry.level {
        LogLevel::Error => ("✗", Color::Red),
        LogLevel::Warn => ("⚠", Color::Yellow),
        LogLevel::Info => ("●", Color::Green),
        LogLevel::Debug => ("○", Color::Blue),
        LogLevel::Trace => ("·", Color::DarkGray),
    };

    let time_len = entry.timestamp.len() + 1; // +1 for space
    let icon_len = 2; // 1 char + 1 space

    // Calculate available width for message
    let msg_width = width.saturating_sub((time_len + icon_len + 4) as u16) as usize; // Extra padding

    // Simple wrapping
    let message = &entry.message;
    let mut lines = Vec::new();

    if message.len() <= msg_width {
        ListItem::new(Line::from(vec![
            Span::styled(
                format!("{} ", entry.timestamp),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(format!("{} ", icon), Style::default().fg(color)),
            Span::styled(message.clone(), Style::default().fg(Color::White)),
        ]))
    } else {
        // First line
        let (first, rest) = message.split_at(std::cmp::min(message.len(), msg_width));
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", entry.timestamp),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(format!("{} ", icon), Style::default().fg(color)),
            Span::styled(first.to_string(), Style::default().fg(Color::White)),
        ]));

        // Subsequent lines
        let chars: Vec<char> = rest.chars().collect();
        for chunk in chars.chunks(msg_width) {
            let s: String = chunk.iter().collect();
            lines.push(Line::from(vec![
                Span::raw(" ".repeat(time_len + icon_len)), // Indent
                Span::styled(s, Style::default().fg(Color::White)),
            ]));
        }
        ListItem::new(Text::from(lines))
    }
}
