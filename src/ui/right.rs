//! Right half: live monitoring + action buttons.

use super::helpers::*;
use crate::app::*;
use ratatui::text::{Line, Span};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Gauge, Paragraph},
};

/// Hover-safe button: background fills ONLY the button's own rect
/// (block styles the whole area incl. border ring; text renders in inner).
pub fn button(
    f: &mut Frame,
    area: Rect,
    label: &str,
    color: Color,
    action: Action,
    click_map: &mut Vec<(Rect, Click)>,
    hover: &Option<Click>,
) {
    let hovered = *hover == Some(Click::Button(action.clone()));
    let block_style = if hovered {
        Style::default().bg(color)
    } else {
        Style::default()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .style(block_style);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let text_style = if hovered {
        Style::default()
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(color).add_modifier(Modifier::BOLD)
    };
    let p = Paragraph::new(label)
        // .alignment(Alignment::Center)
        .style(text_style);
    f.render_widget(p, inner);
    click_map.push((area, Click::Button(action)));
}

pub fn draw_right(f: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" 📊 Live Monitoring")
        .border_style(Style::default().fg(Color::Magenta));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let svc_rows = app.services.len() + usize::from(app.api_status.is_some());
    let rows = Layout::vertical([
        Constraint::Length((svc_rows + 2).clamp(3, 9) as u16),
        Constraint::Length(3), // RAM + CPU gauges (tight, no dead space)
        Constraint::Length(5), // storage: exactly 3 content lines
        Constraint::Length(4), // bandwidth: exactly 2 content lines
        Constraint::Min(3),    // logs
        Constraint::Length(3), // actions
    ])
    .split(inner);

    draw_services(f, rows[0], app);
    draw_gauges(f, rows[1], app);
    draw_storage(f, rows[2], app);
    draw_bandwidth(f, rows[3], app);
    draw_logs_panel(f, rows[4], app);
    draw_actions(f, rows[5], app);
}

fn draw_services(f: &mut Frame, area: Rect, app: &App) {
    let icons = app.icons;
    let mut lines: Vec<Line> = app
        .services
        .iter()
        .map(|s| {
            let (col, st) = if s.active {
                (GREEN, "ACTIVE")
            } else {
                (RED, "DOWN")
            };
            let dot = if s.active {
                icons.dot_ok
            } else {
                icons.dot_bad
            };
            Line::from(vec![
                Span::styled(format!(" {dot} "), Style::default().fg(col)),
                Span::styled(format!("{:<24}", s.name), Style::default().fg(Color::White)),
                Span::styled(st, Style::default().fg(col).add_modifier(Modifier::BOLD)),
            ])
        })
        .collect();

    if let Some((ok, detail)) = &app.api_status {
        let col = if *ok { GREEN } else { RED };
        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", icons.bolt), Style::default().fg(YELLOW)),
            Span::styled(
                format!("{:<24}", "api health"),
                Style::default().fg(Color::White),
            ),
            Span::styled(detail.clone(), Style::default().fg(col)),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            " waiting for first probe…",
            Style::default().fg(DIM),
        )));
    }

    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(format!(" {} Services ", icons.shield))
            .border_style(Style::default().fg(DIM)),
    );
    f.render_widget(p, area);
}

fn draw_gauges(f: &mut Frame, area: Rect, app: &App) {
    let cols =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);
    let m = &app.metrics;
    let icons = app.icons;

    let ram = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(format!(
                    " {} RAM {} / {} ({:.0}%) ",
                    icons.brain,
                    human(m.mem_used),
                    human(m.mem_total),
                    m.mem_percent
                )),
        )
        .gauge_style(Style::default().fg(heat(m.mem_percent)))
        .percent(m.mem_percent.round() as u16);
    f.render_widget(ram, cols[0]);

    let cpu = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(format!(" {} CPU ({:.0}%) ", icons.cpu, m.cpu_percent)),
        )
        .gauge_style(Style::default().fg(heat(m.cpu_percent)))
        .percent(m.cpu_percent.round() as u16);
    f.render_widget(cpu, cols[1]);
}

fn draw_storage(f: &mut Frame, area: Rect, app: &App) {
    let m = &app.metrics;
    let icons = app.icons;
    let bar_w = (area.width.saturating_sub(40)) as usize;
    let disk_ratio = if m.disk_total > 0 {
        m.disk_used as f64 / m.disk_total as f64
    } else {
        0.0
    };
    let db_ratio = if m.disk_total > 0 {
        m.db_bytes as f64 / m.disk_total as f64
    } else {
        0.0
    };
    let up_ratio = if m.disk_total > 0 {
        m.uploads_bytes as f64 / m.disk_total as f64
    } else {
        0.0
    };

    let row = |icon: &str, label: &str, ratio: f64, color: Color, text: String| {
        Line::from(vec![
            Span::styled(
                format!(" {icon} {label:<9}"),
                Style::default().fg(Color::White),
            ),
            Span::styled(bar(ratio, bar_w, app.ascii), Style::default().fg(color)),
            Span::styled(format!("  {text}"), Style::default().fg(DIM)),
        ])
    };

    let lines = vec![
        row(
            icons.db,
            "Database",
            db_ratio.max(if m.db_bytes > 0 { 0.02 } else { 0.0 }),
            Color::Cyan,
            human(m.db_bytes),
        ),
        row(
            icons.r#box,
            "Uploads",
            up_ratio.max(if m.uploads_bytes > 0 { 0.02 } else { 0.0 }),
            Color::Magenta,
            human(m.uploads_bytes),
        ),
        row(
            icons.disk,
            "Disk",
            disk_ratio,
            heat(disk_ratio * 100.0),
            format!(
                "{} / {} ({:.0}%)",
                human(m.disk_used),
                human(m.disk_total),
                disk_ratio * 100.0
            ),
        ),
    ];

    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" 🗄  Storage ")
            .border_style(Style::default().fg(DIM)),
    );
    f.render_widget(p, area);
}

fn draw_bandwidth(f: &mut Frame, area: Rect, app: &App) {
    let icons = app.icons;
    let w = area.width.saturating_sub(26) as usize;
    let lines = vec![
        Line::from(vec![
            Span::styled(
                format!(
                    " {} {:>9}/s ",
                    icons.down,
                    human(app.metrics.rx_rate * 1024)
                ),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                spark(&app.rx_hist, w, app.ascii),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                format!(" {} {:>9}/s ", icons.up, human(app.metrics.tx_rate * 1024)),
                Style::default().fg(YELLOW),
            ),
            Span::styled(
                spark(&app.tx_hist, w, app.ascii),
                Style::default().fg(YELLOW),
            ),
        ]),
    ];
    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" 📡 Bandwidth ")
            .border_style(Style::default().fg(DIM)),
    );
    f.render_widget(p, area);
}

fn draw_logs_panel(f: &mut Frame, area: Rect, app: &App) {
    let visible = area.height.saturating_sub(2) as usize;
    let max_scroll = app.logs.len().saturating_sub(visible);
    let scroll = app.log_scroll.min(max_scroll);

    let lines: Vec<Line> = app
        .logs
        .iter()
        .rev()
        .skip(scroll)
        .take(visible)
        .map(|l| {
            let color = if l.contains("[ERROR]") {
                RED
            } else if l.contains("[WARN]") {
                YELLOW
            } else {
                Color::Gray
            };
            Line::from(Span::styled(format!(" {l}"), Style::default().fg(color)))
        })
        .collect();

    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(format!(" 📜 Activity Log ({} entries) ", app.logs.len()))
            .border_style(Style::default().fg(DIM)),
    );
    f.render_widget(p, area);
}

fn draw_actions(f: &mut Frame, area: Rect, app: &mut App) {
    // ── 4 buttons → exactly 4 columns ────────────────────────────
    let cols = Layout::horizontal(vec![Constraint::Ratio(1, 4); 4]).split(area);
    let armed = app
        .confirm_delete
        .map(|t| t.elapsed().as_secs() < 5)
        .unwrap_or(false);
    let icons = app.icons;
    let del_label = format!(" {} Del ", icons.trash);
    let label: &str = if armed { " ⚠ SURE? " } else { &del_label };

    let App {
        click_map, hover, ..
    } = app;
    button(
        f,
        cols[0],
        &format!(" {} API ", icons.bolt),
        Color::Cyan,
        Action::TestApi,
        click_map,
        hover,
    );
    button(
        f,
        cols[1],
        &format!(" {} Deploy ", icons.down),
        GREEN,
        Action::PullDeploy,
        click_map,
        hover,
    );
    button(
        f,
        cols[2],
        " ↻ Restart ",
        YELLOW,
        Action::Restart,
        click_map,
        hover,
    );
    button(f, cols[3], label, RED, Action::Delete, click_map, hover);
}
