pub mod helpers;
pub mod left;
pub mod right;

use crate::app::App;
use helpers::*;
use ratatui::text::{Line, Span};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Paragraph},
};
use std::time::Instant;

pub fn draw(f: &mut Frame, app: &mut App) {
    app.tick = app.tick.wrapping_add(1);
    app.click_map.clear();
    let area = f.area();

    if app.font_gate {
        draw_gate(f, area, app);
        return;
    }

    let outer = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(14),
        Constraint::Length(3),
    ])
    .split(area);

    draw_header(f, outer[0], app);
    let halves = Layout::horizontal([Constraint::Percentage(46), Constraint::Percentage(54)])
        .split(outer[1]);
    left::draw_left(f, halves[0], app);
    right::draw_right(f, halves[1], app);
    draw_footer(f, outer[2], app);
    draw_notifications(f, area, app);
}

fn draw_gate(f: &mut Frame, area: Rect, app: &App) {
    let busy = if app.gate_busy {
        " ⠋ installing packages…"
    } else {
        ""
    };
    let text = vec![
        Line::from(Span::styled(
            " Your terminal does not appear to support UTF-8.",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            " Icons, bars and graphs may render as garbage characters.",
            Style::default().fg(Color::Gray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " To fix manually on Debian/Ubuntu:",
            Style::default().fg(Color::White),
        )),
        Line::from(Span::styled(
            "   sudo apt install -y locales fonts-noto",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            "   sudo locale-gen en_US.UTF-8",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            "   export LANG=en_US.UTF-8",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  [ i ]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" install required packages now (apt)"),
        ]),
        Line::from(vec![
            Span::styled(
                "  [ c ]",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" continue in ASCII-safe mode"),
        ]),
        Line::from(vec![
            Span::styled(
                "  [ q ]",
                Style::default().fg(RED).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" quit"),
        ]),
        Line::from(Span::styled(busy, Style::default().fg(Color::Yellow))),
    ];
    let p = Paragraph::new(text).alignment(Alignment::Left).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .title(" TERMINAL CAPABILITY CHECK ")
            .border_style(Style::default().fg(Color::Yellow)),
    );
    f.render_widget(p, area);
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    const SPIN: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let status = if app.deploying {
        let s = if app.ascii {
            "..."
        } else {
            &format!("{}", SPIN[app.tick as usize % 10])
        };
        Span::styled(
            format!(" {s} deploying "),
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(" ● ready ", Style::default().fg(GREEN))
    };
    let proj = app
        .deployed
        .as_ref()
        .map(|c| c.project_name.clone())
        .unwrap_or_else(|| "no project".into());
    let line = Line::from(vec![
        Span::styled(format!(" {} ", app.icons.rocket), Style::default()),
        Span::styled(
            "VPS DEPLOYER",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ▐▚ full-stack deployments ▞▌ ", Style::default().fg(DIM)),
        Span::styled(format!(" ⌂ {proj} "), Style::default().fg(Color::Magenta)),
        Span::raw(" "),
        status,
    ]);
    let p = Paragraph::new(line).alignment(Alignment::Center).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .title(" CONTROL CENTER ")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(p, area);
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let p = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" last key: {} ", app.last_key),
            Style::default().fg(Color::Magenta),
        ),
        Span::styled("│", DIM),
        Span::styled(" F(1–7)", Style::default().fg(Color::Cyan)),
        Span::styled(" tabs  ", DIM),
        Span::styled("↑↓/Tab", Style::default().fg(Color::Cyan)),
        Span::styled(" fields  ", DIM),
        Span::styled("Space", Style::default().fg(Color::Cyan)),
        Span::styled(" toggle  ", DIM),
        Span::styled("◀▶", Style::default().fg(Color::Cyan)),
        Span::styled(" stack  ", DIM),
        Span::styled("Wheel", Style::default().fg(Color::Cyan)),
        Span::styled(" scroll  ", DIM),
        Span::styled("Ctrl+Q", Style::default().fg(RED)),
        Span::styled(" quit", DIM),
    ]))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(DIM)),
    );
    f.render_widget(p, area);
}

fn draw_notifications(f: &mut Frame, area: Rect, app: &App) {
    let now = Instant::now();
    let mut y = 1u16;
    for n in app.notifications.iter().rev() {
        if now.duration_since(n.created_at).as_secs() >= 5 {
            continue;
        }
        let w = 44u16.min(area.width.saturating_sub(2));
        let h = 3u16;
        if area.height < y + h + 1 {
            break;
        }
        let rect = Rect::new(
            area.width.saturating_sub(w + 1),
            area.height.saturating_sub(h + 1 + y),
            w,
            h,
        );
        let color = match n.level.as_str() {
            "Error" => RED,
            "Warning" => YELLOW,
            _ => GREEN,
        };
        let p = Paragraph::new(n.message.clone())
            .style(Style::default().fg(color))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(format!(" [{}] ", n.level))
                    .border_style(Style::default().fg(color)),
            );
        f.render_widget(p, rect);
        y += h;
    }
}
