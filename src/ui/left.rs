//! Left half: tab bar, forms, projects list, review screen.

use super::helpers::*;
use crate::app::*;
use crate::config::ProjectConfig;
use crate::ui::right::button;
use ratatui::text::{Line, Span};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Paragraph},
};

pub fn draw_left(f: &mut Frame, area: Rect, app: &mut App) {
    let summary = app.collect_config(); // for the Review tab

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" 📝 Configuration ")
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(inner);

    let App {
        tabs,
        click_map,
        hover,
        left_tab,
        projects,
        projects_focus,
        icons,
        ascii,
        ..
    } = app;

    // tab bar — vertically centered text
    let cols = Layout::horizontal(vec![
        Constraint::Ratio(1, LeftTab::ALL.len() as u32);
        LeftTab::ALL.len()
    ])
    .split(rows[0]);
    for (i, t) in LeftTab::ALL.iter().enumerate() {
        let selected = *left_tab == *t;
        let hovered = *hover == Some(Click::Tab(*t));
        let style = if selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if hovered {
            Style::default().fg(Color::White).bg(Color::DarkGray)
        } else {
            Style::default().fg(Color::Rgb(180, 180, 200))
        };
        let p = Paragraph::new(t.title())
            .alignment(Alignment::Center)
            .alignment(Alignment::Center)
            .style(style);
        f.render_widget(p, cols[i]);
        click_map.push((cols[i], Click::Tab(*t)));
    }

    match *left_tab {
        LeftTab::Projects => draw_projects(
            f,
            rows[1],
            projects,
            *projects_focus,
            click_map,
            hover,
            icons,
            *ascii,
        ),
        LeftTab::Review => draw_review(f, rows[1], &summary, click_map, hover),
        _ => draw_fields(f, rows[1], tabs, *left_tab, click_map),
    }
}

fn draw_fields(
    f: &mut Frame,
    area: Rect,
    tabs: &mut Vec<FormTab>,
    left_tab: LeftTab,
    click_map: &mut Vec<(Rect, Click)>,
) {
    let tab = &mut tabs[left_tab.idx()];
    let n = tab.fields.len();
    let mut cons: Vec<Constraint> = vec![Constraint::Length(3); n];
    cons.push(Constraint::Min(0));
    let chunks = Layout::vertical(cons).split(area);

    for i in 0..n {
        let rect = chunks[i];
        let focused = i == tab.focus;
        click_map.push((
            rect,
            Click::Field {
                tab: left_tab,
                idx: i,
            },
        ));
        let border = if focused {
            Style::default().fg(YELLOW)
        } else {
            Style::default().fg(DIM)
        };
        let arrow = if focused { "▸" } else { " " };

        match &mut tab.fields[i] {
            FormField::Text { label, ta } => {
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(format!(" {arrow} {label} "))
                    .border_style(border);
                ta.set_block(block);
                f.render_widget(&*ta, rect);
            }
            FormField::Toggle { label, hint, value } => {
                let (mark, col, txt) = if *value {
                    ("✓", GREEN, "ON")
                } else {
                    ("✗", DIM, "OFF")
                };
                let mut lines = vec![Line::from(vec![
                    Span::styled(
                        format!("   [{mark}] "),
                        Style::default().fg(col).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(txt, Style::default().fg(col)),
                ])];
                if !hint.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("   {hint}"),
                        Style::default().fg(DIM),
                    )));
                }
                let p = Paragraph::new(lines).alignment(Alignment::Center).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .title(format!(" {arrow} {label} "))
                        .border_style(border),
                );
                f.render_widget(p, rect);
            }
            FormField::Stack { label, value } => {
                let p = Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled(
                            "   ◀  ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            value.label(),
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            "  ▶",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(Span::styled(
                        format!("   build: {}", value.build_command()),
                        Style::default().fg(DIM),
                    )),
                ])
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .title(format!(" {arrow} {label} "))
                        .border_style(border),
                );
                f.render_widget(p, rect);
            }
        }
    }
}

fn draw_projects(
    f: &mut Frame,
    area: Rect,
    projects: &[ProjectConfig],
    focus: usize,
    click_map: &mut Vec<(Rect, Click)>,
    hover: &Option<Click>,
    icons: &crate::theme::Icons,
    ascii: bool,
) {
    let rows = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(area);

    let inner_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(format!(
            " {} Projects on this VPS ({}) ",
            icons.r#box,
            projects.len()
        ))
        .border_style(Style::default().fg(Color::Magenta));
    let inner = inner_block.inner(rows[0]);
    f.render_widget(inner_block, rows[0]);

    if projects.is_empty() {
        let p = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled("  No projects yet.", Style::default().fg(DIM))),
            Line::from(Span::styled(
                "  Fill the forms (F1–F5) and hit Deploy 🚀",
                Style::default().fg(DIM),
            )),
        ]);
        f.render_widget(p, inner);
    }

    for (i, p) in projects.iter().enumerate() {
        if i as u16 >= inner.height {
            break;
        }
        let row_rect = Rect::new(inner.x, inner.y + i as u16, inner.width, 1);
        click_map.push((row_rect, Click::Project(i)));

        let selected = i == focus;
        let hovered = *hover == Some(Click::Project(i));
        let domain = if p.domain.is_empty() {
            "no-domain".to_string()
        } else {
            p.domain.clone()
        };
        let text = format!(
            " {} {:<18} {:<8} {:<24} {}",
            if selected { "▶" } else { " " },
            p.project_name,
            p.stack.short(),
            domain,
            p.target_dir
        );
        let style = if selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD)
        } else if hovered {
            Style::default().fg(Color::White).bg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };
        let line = Paragraph::new(text).style(style);
        f.render_widget(line, row_rect);
    }

    let cols = Layout::horizontal(vec![Constraint::Ratio(1, 4); 4]).split(rows[1]);
    let _ = ascii;
    button(
        f,
        cols[0],
        &format!(" {} Load/Edit ", icons.enter),
        Color::Cyan,
        Action::LoadProject,
        click_map,
        hover,
    );
    button(
        f,
        cols[1],
        &format!(" {} Backup ", icons.backup),
        GREEN,
        Action::BackupNow,
        click_map,
        hover,
    );
    button(
        f,
        cols[2],
        &format!(" {} Delete ", icons.trash),
        RED,
        Action::DeleteProjectSel,
        click_map,
        hover,
    );
    button(
        f,
        cols[3],
        &format!(" {} New ", icons.plus),
        YELLOW,
        Action::NewProject,
        click_map,
        hover,
    );
}

fn draw_review(
    f: &mut Frame,
    area: Rect,
    cfg: &ProjectConfig,
    click_map: &mut Vec<(Rect, Click)>,
    hover: &Option<Click>,
) {
    let rows = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(area);
    let be = if cfg.backend_repo.is_empty() {
        "—"
    } else {
        "✓"
    };
    let fe = if cfg.frontend_repo.is_empty() {
        "—"
    } else {
        "✓"
    };

    let summary: Vec<Line> = vec![
        Line::from(vec![
            Span::styled(" Project   : ", Style::default().fg(DIM)),
            Span::styled(
                format!("{}  ({})", cfg.project_name, cfg.stack.label()),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Repos     : ", Style::default().fg(DIM)),
            Span::styled(
                format!("backend {be} · frontend {fe} · branch '{}'", cfg.branch),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Dirs      : ", Style::default().fg(DIM)),
            Span::styled(
                format!(
                    "{}  ·  db {}  ·  uploads {}",
                    cfg.target_dir, cfg.db_dir, cfg.uploads_dir
                ),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Network   : ", Style::default().fg(DIM)),
            Span::styled(
                format!(
                    "port {} · domain {} · health {}",
                    cfg.backend_port,
                    if cfg.domain.is_empty() {
                        "—"
                    } else {
                        &cfg.domain
                    },
                    cfg.health_path
                ),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Backups   : ", Style::default().fg(DIM)),
            Span::styled(
                format!(
                    "{} · keep {} · cron {}",
                    if cfg.backup_enabled { "on" } else { "off" },
                    cfg.backup_retention,
                    if cfg.backup_cron {
                        "daily 03:00"
                    } else {
                        "off"
                    }
                ),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            " Staged files — edit with vim, then use 'My edits':",
            Style::default().fg(YELLOW),
        )),
        Line::from(Span::styled(
            format!("   {}/deploy.sh", cfg.staging_dir()),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            format!("   {}/{}.service", cfg.staging_dir(), cfg.service_name()),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            format!(
                "   {}/nginx-{}.conf",
                cfg.staging_dir(),
                if cfg.domain.is_empty() {
                    "<domain>"
                } else {
                    &cfg.domain
                }
            ),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            format!("   {}/backup.sh", cfg.staging_dir()),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            format!("   {}/DOCUMENTATION.md", cfg.staging_dir()),
            Style::default().fg(Color::Cyan),
        )),
    ];

    let p = Paragraph::new(summary).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" 🚦 Review & Deploy ")
            .border_style(Style::default().fg(GREEN)),
    );
    f.render_widget(p, rows[0]);

    // ── 4 buttons → exactly 4 columns ────────────────────────────
    let cols = Layout::horizontal(vec![Constraint::Ratio(1, 4); 4]).split(rows[1]);
    button(
        f,
        cols[0],
        " 🚀 Deploy ",
        GREEN,
        Action::DeployRegen,
        click_map,
        hover,
    );
    button(
        f,
        cols[1],
        " 📥 My edits ",
        Color::Cyan,
        Action::DeployStaged,
        click_map,
        hover,
    );
    button(
        f,
        cols[2],
        " 📜 Files ",
        YELLOW,
        Action::WriteFiles,
        click_map,
        hover,
    );
    button(
        f,
        cols[3],
        " 📖 Docs ",
        Color::Magenta,
        Action::GetDocs,
        click_map,
        hover,
    );
}
