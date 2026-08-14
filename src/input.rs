//! Keyboard + mouse handling and action dispatch.

use crate::actions;
use crate::app::*;
use crate::config::{self, ProjectConfig};
use crate::docs;
use crate::theme;
use crossterm::event::MouseEvent;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use std::time::Instant;
use tokio::sync::{mpsc, watch};

type Tx<'a> = &'a mpsc::Sender<AppMessage>;
type CfgTx<'a> = &'a watch::Sender<Option<ProjectConfig>>;

pub async fn handle_key(
    app: &mut App,
    key: KeyEvent,
    tx: Tx<'_>,
    cfg_tx: CfgTx<'_>,
) -> anyhow::Result<bool> {
    app.last_key = format!("{:?}", key.code);

    if app.font_gate {
        return gate_key(app, key, tx).await;
    }

    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('c') if ctrl => return Ok(true),
        KeyCode::F(n @ 1..=7) => {
            app.left_tab = LeftTab::ALL[(n - 1) as usize];
            return Ok(false);
        }
        KeyCode::PageDown => {
            app.left_tab = app.left_tab.next();
            return Ok(false);
        }
        KeyCode::PageUp => {
            app.left_tab = app.left_tab.prev();
            return Ok(false);
        }
        _ => {}
    }

    // ── NEW: number keys 1–7 switch tabs ──────────────────────────
    if let KeyCode::Char(c @ '1'..='7') = key.code {
        let typing_in_text = match app.left_tab {
            LeftTab::Projects | LeftTab::Review => false,
            _ => {
                let tab = &app.tabs[app.left_tab.idx()];
                matches!(tab.fields.get(tab.focus), Some(FormField::Text { .. }))
            }
        };
        if !typing_in_text {
            app.left_tab = LeftTab::ALL[(c as u8 - b'1') as usize];
            return Ok(false);
        }
    }
    // ───────────────────────────────────────────────────────────────

    match app.left_tab {
        LeftTab::Projects => {
            projects_key(app, key, tx, cfg_tx).await;
            return Ok(false);
        }
        LeftTab::Review => {
            match key.code {
                KeyCode::Enter => trigger(app, Action::DeployRegen, tx, cfg_tx).await,
                KeyCode::Char('e') => trigger(app, Action::DeployStaged, tx, cfg_tx).await,
                KeyCode::Char('w') => trigger(app, Action::WriteFiles, tx, cfg_tx).await,
                KeyCode::Char('g') => trigger(app, Action::GetDocs, tx, cfg_tx).await,
                _ => {}
            }
            return Ok(false);
        }
        _ => {}
    }

    let tab_idx = app.left_tab.idx();
    let len = app.tabs[tab_idx].fields.len();

    match key.code {
        KeyCode::Up | KeyCode::BackTab => {
            app.tabs[tab_idx].focus = (app.tabs[tab_idx].focus + len - 1) % len;
            return Ok(false);
        }
        KeyCode::Down | KeyCode::Tab => {
            app.tabs[tab_idx].focus = (app.tabs[tab_idx].focus + 1) % len;
            return Ok(false);
        }
        _ => {}
    }

    let mut next_focus: Option<usize> = None;
    {
        let tab = &mut app.tabs[tab_idx];
        let focus = tab.focus;
        match &mut tab.fields[focus] {
            FormField::Toggle { value, .. } => {
                if matches!(
                    key.code,
                    KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right
                ) {
                    *value = !*value;
                }
            }
            FormField::Stack { value, .. } => match key.code {
                KeyCode::Enter | KeyCode::Right => *value = value.next(),
                KeyCode::Left => *value = value.prev(),
                _ => {}
            },
            FormField::Text { ta, .. } => {
                if key.code == KeyCode::Enter {
                    next_focus = Some((focus + 1) % len);
                } else {
                    ta.input(key);
                }
            }
        }
    }
    if let Some(nf) = next_focus {
        app.tabs[tab_idx].focus = nf;
    }
    Ok(false)
}

async fn projects_key(app: &mut App, key: KeyEvent, tx: Tx<'_>, cfg_tx: CfgTx<'_>) {
    let n = app.projects.len();
    match key.code {
        KeyCode::Up | KeyCode::Char('k') if n > 0 => {
            app.projects_focus = (app.projects_focus + n - 1) % n
        }
        KeyCode::Down | KeyCode::Char('j') if n > 0 => {
            app.projects_focus = (app.projects_focus + 1) % n
        }
        KeyCode::Enter => trigger(app, Action::LoadProject, tx, cfg_tx).await,
        KeyCode::Char('b') => trigger(app, Action::BackupNow, tx, cfg_tx).await,
        KeyCode::Char('d') => trigger(app, Action::DeleteProjectSel, tx, cfg_tx).await,
        KeyCode::Char('n') => trigger(app, Action::NewProject, tx, cfg_tx).await,
        KeyCode::Char('g') => trigger(app, Action::GetDocs, tx, cfg_tx).await,
        _ => {}
    }
}

async fn gate_key(app: &mut App, key: KeyEvent, tx: Tx<'_>) -> anyhow::Result<bool> {
    match key.code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Char('c') => {
            app.ascii = true;
            app.icons = theme::ASCII;
            app.font_gate = false;
            app.push_log("[INFO] continuing in ASCII-safe mode".into());
        }
        KeyCode::Char('i') if !app.gate_busy => {
            app.gate_busy = true;
            let tx = tx.clone();
            tokio::spawn(async move {
                let res = actions::sh(
                    "apt-get install -y locales fonts-noto fonts-noto-color-emoji >/dev/null 2>&1; \
                     locale-gen en_US.UTF-8 >/dev/null 2>&1; \
                     echo installed",
                )
                .await;
                let _ = tx.send(AppMessage::GateDone(
                    res.map(|_| "Packages installed — reopen the TUI in a UTF-8 terminal (LANG=en_US.UTF-8)".into())
                )).await;
            });
        }
        _ => {}
    }
    Ok(false)
}

pub async fn handle_mouse(
    app: &mut App,
    m: MouseEvent,
    tx: Tx<'_>,
    cfg_tx: CfgTx<'_>,
) -> anyhow::Result<()> {
    let find = |app: &App| -> Option<Click> {
        app.click_map
            .iter()
            .rev()
            .find(|(r, _)| contains(r, m.column, m.row))
            .map(|(_, c)| c.clone())
    };
    match m.kind {
        MouseEventKind::Moved => {
            app.hover = find(app);
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(click) = find(app) {
                match click {
                    Click::Tab(t) => app.left_tab = t,
                    Click::Field { tab, idx } => {
                        app.left_tab = tab;
                        if let Some(t) = app.tabs.get_mut(tab.idx()) {
                            if idx < t.fields.len() {
                                t.focus = idx;
                                if let FormField::Toggle { value, .. } = &mut t.fields[idx] {
                                    *value = !*value;
                                }
                            }
                        }
                    }
                    Click::Project(i) => {
                        app.projects_focus = i;
                    }
                    Click::Button(a) => trigger(app, a, tx, cfg_tx).await,
                }
            }
        }
        MouseEventKind::ScrollUp => {
            app.log_scroll = (app.log_scroll + 2).min(app.logs.len().saturating_sub(1));
        }
        MouseEventKind::ScrollDown => {
            app.log_scroll = app.log_scroll.saturating_sub(2);
        }
        _ => {}
    }
    Ok(())
}

fn contains(r: &Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
}

// ── action dispatch ────────────────────────────────────────────────
pub async fn trigger(app: &mut App, action: Action, tx: Tx<'_>, cfg_tx: CfgTx<'_>) {
    match action {
        Action::DeployRegen | Action::DeployStaged => {
            let cfg = app.collect_config();
            if let Err(e) = config::validate(&cfg) {
                app.notify(e, "Error");
                return;
            }
            let use_staged = action == Action::DeployStaged;
            app.deploying = true;
            app.deployed = Some(cfg.clone());
            let _ = cfg_tx.send(Some(cfg.clone()));
            let tx = tx.clone();
            tokio::spawn(async move { actions::deploy_project(cfg, use_staged, tx).await });
        }
        Action::WriteFiles => {
            let cfg = app.collect_config();
            if cfg.target_dir.is_empty() {
                app.notify("Deploy Directory is required first".into(), "Error");
                return;
            }
            let _ = actions::run("mkdir", &["-p", &cfg.target_dir]).await;
            match actions::write_staged(&cfg).await {
                Ok(_) => app.notify(
                    format!("files staged at {} — review with vim", cfg.staging_dir()),
                    "Info",
                ),
                Err(e) => app.notify(e, "Error"),
            }
        }
        Action::PullDeploy => {
            if let Some(cfg) = app.deployed.clone() {
                app.deploying = true;
                app.push_log("[INFO] ⇩ running deploy.sh (pull + build)…".into());
                let tx = tx.clone();
                tokio::spawn(async move {
                    match actions::sh(&format!("cd '{}' && bash ./deploy.sh 2>&1", cfg.target_dir))
                        .await
                    {
                        Ok(out) => {
                            for line in out.lines().rev().take(15) {
                                let _ = tx.send(AppMessage::Log(format!("[INFO] {line}"))).await;
                            }
                            let _ = tx
                                .send(AppMessage::DeployDone(Ok("Pull & deploy finished".into())))
                                .await;
                        }
                        Err(e) => {
                            let _ = tx.send(AppMessage::DeployDone(Err(e))).await;
                        }
                    }
                });
            } else {
                app.notify("Deploy a project first".into(), "Warning");
            }
        }
        Action::TestApi => {
            if let Some(cfg) = app.deployed.clone() {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let (ok, detail) = actions::check_api_health(&cfg).await;
                    let _ = tx.send(AppMessage::ApiHealth { ok, detail }).await;
                });
            } else {
                app.notify("No deployed project".into(), "Warning");
            }
        }
        Action::Restart => {
            if let Some(cfg) = app.deployed.clone() {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let svc = cfg.service_name();
                    let r1 = actions::run("systemctl", &["restart", &svc]).await;
                    let r2 = actions::run("systemctl", &["reload", "nginx"]).await;
                    let msg = if r1.is_ok() && r2.is_ok() {
                        Ok("Services restarted ↻".into())
                    } else {
                        Err(format!(
                            "restart failed: {} | {}",
                            r1.err().unwrap_or_else(|| "ok".into()),
                            r2.err().unwrap_or_else(|| "ok".into())
                        ))
                    };
                    let _ = tx.send(AppMessage::Toast(msg)).await;
                });
            } else {
                app.notify("No deployed project".into(), "Warning");
            }
        }
        Action::Delete => delete_flow(app, app.deployed.clone(), tx, cfg_tx).await,
        Action::DeleteProjectSel => {
            let target = app.projects.get(app.projects_focus).cloned();
            delete_flow(app, target, tx, cfg_tx).await;
        }
        Action::LoadProject => {
            if let Some(cfg) = app.projects.get(app.projects_focus).cloned() {
                let _ = cfg_tx.send(Some(cfg.clone()));
                app.load_project(cfg);
            } else {
                app.notify("No project selected".into(), "Warning");
            }
        }
        Action::BackupNow => {
            let cfg = app
                .projects
                .get(app.projects_focus)
                .cloned()
                .or_else(|| app.deployed.clone());
            if let Some(cfg) = cfg {
                app.notify(format!("Backing up '{}'…", cfg.project_name), "Info");
                let tx = tx.clone();
                tokio::spawn(async move { actions::run_backup(cfg, tx).await });
            } else {
                app.notify("No project to back up".into(), "Warning");
            }
        }
        Action::NewProject => app.reset_form(),
        Action::GetDocs => {
            // Prefer the selected/deployed project; fall back to the current form
            let cfg = if app.left_tab == LeftTab::Projects {
                app.projects
                    .get(app.projects_focus)
                    .cloned()
                    .or_else(|| app.deployed.clone())
            } else if let Some(d) = app.deployed.clone() {
                Some(d)
            } else {
                let c = app.collect_config();
                if c.target_dir.is_empty() {
                    None
                } else {
                    Some(c)
                }
            };

            match cfg {
                Some(cfg) => match docs::write_docs(&cfg).await {
                    Ok(path) => {
                        app.push_log(format!("[INFO] 📖 docs written to {path}"));
                        app.notify(format!("📖 docs: {path}"), "Info");
                    }
                    Err(e) => app.notify(e, "Error"),
                },
                None => app.notify("Fill at least the Deploy Directory first".into(), "Warning"),
            }
        }
    }
}

async fn delete_flow(app: &mut App, target: Option<ProjectConfig>, tx: Tx<'_>, cfg_tx: CfgTx<'_>) {
    let Some(cfg) = target else {
        app.notify("Nothing to delete".into(), "Warning");
        return;
    };
    let armed = app
        .confirm_delete
        .map(|t| t.elapsed().as_secs() < 5)
        .unwrap_or(false)
        && app
            .pending_delete
            .as_ref()
            .map(|p| p.project_name == cfg.project_name)
            .unwrap_or(false);

    if armed {
        app.confirm_delete = None;
        app.pending_delete = None;
        app.deploying = true;
        if app
            .deployed
            .as_ref()
            .map(|d| d.project_name == cfg.project_name)
            .unwrap_or(false)
        {
            app.deployed = None;
            let _ = cfg_tx.send(None);
        }
        let tx = tx.clone();
        tokio::spawn(actions::delete_project(cfg, tx));
    } else {
        app.pending_delete = Some(cfg.clone());
        app.confirm_delete = Some(Instant::now());
        app.notify(
            format!("⚠ press DELETE again to erase '{}'", cfg.project_name),
            "Warning",
        );
    }
}
