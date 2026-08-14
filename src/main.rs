mod actions;
mod app;
mod config;
mod docs;
mod input;
mod monitor;
mod templates;
mod theme;
mod ui;

use app::*;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::panic;
use std::time::Duration;
use tokio::sync::{mpsc, watch};

fn enable_terminal() -> io::Result<()> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    Ok(())
}

fn disable_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

#[tokio::main]
async fn main() {
    // If ANYTHING panics, give the terminal back first, then show the panic.
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_terminal();
        default_hook(info);
    }));

    if let Err(err) = enable_terminal() {
        eprintln!("failed to initialise terminal: {err}");
        return;
    }

    let result = run_app().await;

    // Always restore, on success AND error.
    let _ = disable_terminal();

    if let Err(err) = result {
        eprintln!("\n=== VPS Deployer exited with an error ===\n{err:?}\n");
        std::process::exit(1);
    }
}

async fn run_app() -> anyhow::Result<()> {
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    if !theme::looks_utf8() {
        app.font_gate = true;
    }

    let (tx, mut rx) = mpsc::channel::<AppMessage>(64);
    let (cfg_tx, cfg_rx) = watch::channel::<Option<config::ProjectConfig>>(None);
    tokio::spawn(monitor::monitor(cfg_rx, tx.clone()));

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        if event::poll(Duration::from_millis(120))? {
            match event::read()? {
                Event::Key(k) if k.kind == event::KeyEventKind::Press => {
                    if input::handle_key(&mut app, k, &tx, &cfg_tx).await? {
                        break;
                    }
                }
                Event::Mouse(m) => input::handle_mouse(&mut app, m, &tx, &cfg_tx).await?,
                _ => {}
            }
        }

        while let Ok(msg) = rx.try_recv() {
            match msg {
                AppMessage::Metrics(mut m) => {
                    m.db_bytes = app.metrics.db_bytes;
                    m.uploads_bytes = app.metrics.uploads_bytes;
                    app.metrics = m;
                    app.rx_hist.push(app.metrics.rx_rate);
                    app.tx_hist.push(app.metrics.tx_rate);
                    if app.rx_hist.len() > HISTORY {
                        app.rx_hist.remove(0);
                    }
                    if app.tx_hist.len() > HISTORY {
                        app.tx_hist.remove(0);
                    }
                }
                AppMessage::Heavy {
                    services,
                    db_bytes,
                    uploads_bytes,
                } => {
                    app.services = services;
                    app.metrics.db_bytes = db_bytes;
                    app.metrics.uploads_bytes = uploads_bytes;
                }
                AppMessage::ApiHealth { ok, detail } => {
                    app.push_log(format!(
                        "{} API health: {detail}",
                        if ok { "[INFO]" } else { "[WARN]" }
                    ));
                    app.api_status = Some((ok, detail));
                }
                AppMessage::Log(l) => app.push_log(l),
                AppMessage::Notify { msg, level } => app.notify(msg, &level),
                AppMessage::DeployDone(res) => {
                    app.deploying = false;
                    app.refresh_projects();
                    match res {
                        Ok(m) => {
                            app.notify(m, "Info");
                            app.push_log("[INFO] ✅ deploy done".into());
                        }
                        Err(e) => {
                            app.notify(e.clone(), "Error");
                            app.push_log(format!("[ERROR] {e}"));
                        }
                    }
                }
                AppMessage::Deleted(res) => {
                    app.deploying = false;
                    app.deployed = None;
                    app.api_status = None;
                    app.services.clear();
                    let _ = cfg_tx.send(None);
                    app.refresh_projects();
                    match res {
                        Ok(m) => app.notify(m, "Info"),
                        Err(e) => app.notify(e, "Error"),
                    }
                }
                AppMessage::Toast(res) => match res {
                    Ok(m) => {
                        app.notify(m.clone(), "Info");
                        app.push_log(format!("[INFO] {m}"));
                    }
                    Err(e) => {
                        app.notify(e.clone(), "Error");
                        app.push_log(format!("[ERROR] {e}"));
                    }
                },
                AppMessage::GateDone(res) => {
                    app.gate_busy = false;
                    app.font_gate = false;
                    match res {
                        Ok(m) => app.notify(m, "Info"),
                        Err(e) => {
                            app.notify(e, "Warning");
                            app.ascii = true;
                            app.icons = theme::ASCII;
                        }
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
