//! All shell/system operations: deploy, install, delete, backup, health.

use crate::app::{AppMessage, ServiceStatus};
use crate::config::{self, ProjectConfig};
use crate::templates;
use std::path::Path;
use tokio::process::Command;
use tokio::sync::mpsc::Sender;

/// Clone a repo on the given branch; fall back to the default branch.
async fn clone_repo(repo: &str, branch: &str, dest: &str) -> Result<String, String> {
    match run("git", &["clone", "-b", branch, repo, dest]).await {
        ok @ Ok(_) => ok,
        Err(_) => run("git", &["clone", repo, dest]).await,
    }
}

pub async fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(cmd)
        .args(args)
        .output()
        .await
        .map_err(|e| format!("{cmd}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub async fn sh(line: &str) -> Result<String, String> {
    run("bash", &["-c", line]).await
}

async fn say(tx: &Sender<AppMessage>, msg: impl Into<String>) {
    let _ = tx.send(AppMessage::Log(msg.into())).await;
}

// ── Staged files (vim-editable) ────────────────────────────────────
pub async fn write_staged(cfg: &ProjectConfig) -> Result<(), String> {
    let staging = cfg.staging_dir();
    tokio::fs::create_dir_all(&staging)
        .await
        .map_err(|e| format!("create {staging}: {e}"))?;

    let deploy = format!("{staging}/deploy.sh");
    tokio::fs::write(&deploy, templates::deploy_sh(cfg))
        .await
        .map_err(|e| e.to_string())?;
    let _ = run("chmod", &["+x", &deploy]).await;

    let unit = format!("{staging}/{}.service", cfg.service_name());
    tokio::fs::write(&unit, templates::backend_service(cfg))
        .await
        .map_err(|e| e.to_string())?;

    if !cfg.domain.is_empty() {
        let conf = format!("{staging}/nginx-{}.conf", cfg.domain);
        tokio::fs::write(&conf, templates::nginx_conf(cfg))
            .await
            .map_err(|e| e.to_string())?;
    }
    if cfg.backup_enabled {
        let bk = format!("{staging}/backup.sh");
        tokio::fs::write(&bk, templates::backup_sh(cfg))
            .await
            .map_err(|e| e.to_string())?;
        let _ = run("chmod", &["+x", &bk]).await;
    }
    Ok(())
}

// ── Install staged files into the system ───────────────────────────
async fn install_staged(cfg: &ProjectConfig, tx: &Sender<AppMessage>) -> Result<(), String> {
    let staging = cfg.staging_dir();
    let svc = cfg.service_name();

    // systemd
    say(tx, "[INFO] installing systemd unit …").await;
    let unit_src = format!("{staging}/{svc}.service");
    let unit_dst = format!("/etc/systemd/system/{svc}.service");
    tokio::fs::copy(&unit_src, &unit_dst)
        .await
        .map_err(|e| format!("install unit: {e} (run the TUI with sudo?)"))?;
    run("systemctl", &["daemon-reload"]).await?;
    run("systemctl", &["enable", "--now", &svc])
        .await
        .map_err(|e| format!("enable {svc}: {e}"))?;

    // nginx
    if !cfg.domain.is_empty() {
        say(
            tx,
            format!("[INFO] installing nginx vhost for {} …", cfg.domain),
        )
        .await;
        let src = format!("{staging}/nginx-{}.conf", cfg.domain);
        let avail = format!("/etc/nginx/sites-available/{}", cfg.domain);
        let enabled = format!("/etc/nginx/sites-enabled/{}", cfg.domain);
        tokio::fs::copy(&src, &avail)
            .await
            .map_err(|e| format!("install nginx conf: {e}"))?;
        let _ = run("ln", &["-sf", &avail, &enabled]).await;
        match run("nginx", &["-t"]).await {
            Ok(_) => {
                let _ = run("systemctl", &["reload", "nginx"]).await;
            }
            Err(e) => say(tx, format!("[WARN] nginx -t failed: {e}")).await,
        }
        if !cfg.cert_email.is_empty() {
            say(tx, "[INFO] requesting Let's Encrypt certificate …").await;
            match run(
                "certbot",
                &[
                    "--nginx",
                    "-d",
                    &cfg.domain,
                    "--non-interactive",
                    "--agree-tos",
                    "--redirect",
                    "-m",
                    &cfg.cert_email,
                ],
            )
            .await
            {
                Ok(_) => say(tx, "[INFO] ✅ SSL installed").await,
                Err(e) => say(tx, format!("[WARN] certbot: {e}")).await,
            }
        } else {
            say(tx, "[WARN] no certbot email → skipping SSL").await;
        }
    }

    // ufw
    say(tx, "[INFO] configuring UFW …").await;
    if cfg.ufw_web {
        let _ = run("ufw", &["allow", "80/tcp"]).await;
        let _ = run("ufw", &["allow", "443/tcp"]).await;
    }
    if cfg.ufw_backend {
        let _ = run("ufw", &["allow", &format!("{}/tcp", cfg.backend_port)]).await;
    }
    for p in cfg.extra_ports.split(',') {
        let p = p.trim();
        if !p.is_empty() {
            let _ = run("ufw", &["allow", p]).await;
        }
    }

    // backup cron
    if cfg.backup_enabled && cfg.backup_cron {
        install_cron(cfg).await.ok();
    } else {
        remove_cron(cfg).await.ok();
    }
    Ok(())
}

// ── Full deployment ────────────────────────────────────────────────
pub async fn deploy_project(cfg: ProjectConfig, use_staged: bool, tx: Sender<AppMessage>) {
    say(&tx, format!("[INFO] 🚀 deploying '{}' …", cfg.project_name)).await;

    // dirs
    for d in [
        &cfg.target_dir,
        &cfg.db_dir,
        &cfg.uploads_dir,
        &cfg.backup_dir,
    ] {
        if !d.is_empty() {
            let _ = run("mkdir", &["-p", d]).await;
        }
    }

    // clone / pull
    let be_dir = format!("{}/backend", cfg.target_dir);
    let fe_dir = format!("{}/frontend", cfg.target_dir);
    if !cfg.backend_repo.is_empty() {
        if Path::new(&format!("{be_dir}/.git")).exists() {
            say(&tx, "[INFO] backend exists → git pull").await;
            let _ = sh(&format!("cd '{be_dir}' && git pull --ff-only || true")).await;
        } else {
            say(&tx, "[INFO] cloning backend …").await;
            if let Err(e) = clone_repo(&cfg.backend_repo, &cfg.branch, &be_dir).await {
                let _ = tx
                    .send(AppMessage::DeployDone(Err(format!(
                        "backend clone failed: {e}"
                    ))))
                    .await;
                return;
            }
        }
    }
    if !cfg.frontend_repo.is_empty() {
        if Path::new(&format!("{fe_dir}/.git")).exists() {
            say(&tx, "[INFO] frontend exists → git pull").await;
            let _ = sh(&format!("cd '{fe_dir}' && git pull --ff-only || true")).await;
        } else {
            say(&tx, "[INFO] cloning frontend …").await;
            if let Err(e) = clone_repo(&cfg.frontend_repo, &cfg.branch, &fe_dir).await {
                let _ = tx
                    .send(AppMessage::DeployDone(Err(format!(
                        "frontend clone failed: {e}"
                    ))))
                    .await;
                return;
            }
        }
    }

    // initial build
    if !cfg.backend_repo.is_empty() {
        say(
            &tx,
            format!("[INFO] building backend ({}) …", cfg.stack.label()),
        )
        .await;
        if let Err(e) = sh(&format!("cd '{be_dir}' && {}", cfg.stack.build_command())).await {
            let _ = tx
                .send(AppMessage::DeployDone(Err(format!(
                    "backend build failed: {e}"
                ))))
                .await;
            return;
        }
    }

    // staged files
    let staging_ok =
        if use_staged && Path::new(&format!("{}/deploy.sh", cfg.staging_dir())).exists() {
            say(&tx, "[INFO] using YOUR staged files (vim edits preserved)").await;
            true
        } else {
            say(&tx, "[INFO] generating staged files …").await;
            match write_staged(&cfg).await {
                Ok(_) => true,
                Err(e) => {
                    let _ = tx.send(AppMessage::DeployDone(Err(e))).await;
                    return;
                }
            }
        };
    if !staging_ok {
        return;
    }

    if let Err(e) = install_staged(&cfg, &tx).await {
        let _ = tx.send(AppMessage::DeployDone(Err(e))).await;
        return;
    }

    if let Err(e) = config::save_project(&cfg) {
        say(&tx, format!("[WARN] could not save project file: {e}")).await;
    }
    let _ = tx
        .send(AppMessage::DeployDone(Ok("Deployment finished 🎉".into())))
        .await;
}

// ── Backup ─────────────────────────────────────────────────────────
pub async fn run_backup(cfg: ProjectConfig, tx: Sender<AppMessage>) {
    if cfg.backup_dir.is_empty() {
        let _ = tx
            .send(AppMessage::Toast(Err("Backup directory not set".into())))
            .await;
        return;
    }
    let _ = run("mkdir", &["-p", &cfg.backup_dir]).await;
    let script = format!("{}/backup.sh", cfg.staging_dir());
    if Path::new(&script).exists() {
        // respect user edits — only regenerate when missing
    } else if let Err(e) = write_staged(&cfg).await {
        let _ = tx.send(AppMessage::Toast(Err(e))).await;
        return;
    }
    match sh(&format!("bash '{script}'")).await {
        Ok(out) => {
            let last = out.lines().last().unwrap_or("done").to_string();
            let _ = tx
                .send(AppMessage::Toast(Ok(format!("Backup OK — {last}"))))
                .await;
        }
        Err(e) => {
            let _ = tx
                .send(AppMessage::Toast(Err(format!("backup failed: {e}"))))
                .await;
        }
    }
}

async fn install_cron(cfg: &ProjectConfig) -> Result<(), String> {
    let line = format!(
        "0 3 * * * /usr/bin/env bash '{}/backup.sh' >> '{}/cron.log' 2>&1 # vps-deployer:{}",
        cfg.staging_dir(),
        cfg.backup_dir,
        cfg.project_name
    );
    sh(&format!(
        "(crontab -l 2>/dev/null | grep -v 'vps-deployer:{}' || true; echo '{}') | crontab -",
        cfg.project_name, line
    ))
    .await
    .map(|_| ())
}

async fn remove_cron(cfg: &ProjectConfig) -> Result<(), String> {
    sh(&format!(
        "(crontab -l 2>/dev/null | grep -v 'vps-deployer:{}' || true) | crontab -",
        cfg.project_name
    ))
    .await
    .map(|_| ())
}

// ── Delete everything ──────────────────────────────────────────────
pub async fn delete_project(cfg: ProjectConfig, tx: Sender<AppMessage>) {
    say(&tx, "[INFO] 🧹 removing every footprint …").await;
    let svc = cfg.service_name();
    let _ = run("systemctl", &["stop", &svc]).await;
    let _ = run("systemctl", &["disable", &svc]).await;
    let _ = tokio::fs::remove_file(format!("/etc/systemd/system/{svc}.service")).await;
    let _ = run("systemctl", &["daemon-reload"]).await;
    let _ = run("systemctl", &["reset-failed"]).await;

    if !cfg.domain.is_empty() {
        let _ = tokio::fs::remove_file(format!("/etc/nginx/sites-available/{}", cfg.domain)).await;
        let _ = tokio::fs::remove_file(format!("/etc/nginx/sites-enabled/{}", cfg.domain)).await;
        let _ = run(
            "certbot",
            &["delete", "--cert-name", &cfg.domain, "--non-interactive"],
        )
        .await;
        let _ = run("nginx", &["-t"]).await;
        let _ = run("systemctl", &["reload", "nginx"]).await;
    }

    let _ = remove_cron(&cfg).await;
    if cfg.ufw_backend {
        let _ = run(
            "ufw",
            &["delete", "allow", &format!("{}/tcp", cfg.backend_port)],
        )
        .await;
    }
    for p in cfg.extra_ports.split(',') {
        let p = p.trim();
        if !p.is_empty() {
            let _ = run("ufw", &["delete", "allow", p]).await;
        }
    }

    let _ = tokio::fs::remove_dir_all(&cfg.target_dir).await;
    if !cfg.db_dir.is_empty() {
        let _ = tokio::fs::remove_dir_all(&cfg.db_dir).await;
    }
    if !cfg.uploads_dir.is_empty() {
        let _ = tokio::fs::remove_dir_all(&cfg.uploads_dir).await;
    }
    if !cfg.backup_dir.is_empty() {
        let _ = tokio::fs::remove_dir_all(&cfg.backup_dir).await;
    }
    config::remove_project_file(&cfg.project_name);

    let _ = tx
        .send(AppMessage::Deleted(Ok(
            "Project removed without a trace 🧹".into(),
        )))
        .await;
}

// ── Health & monitoring helpers ────────────────────────────────────
pub async fn check_api_health(cfg: &ProjectConfig) -> (bool, String) {
    let path = if cfg.health_path.starts_with('/') {
        cfg.health_path.clone()
    } else {
        format!("/{}", cfg.health_path)
    };
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .build()
    {
        Ok(c) => c,
        Err(e) => return (false, e.to_string()),
    };
    match client
        .get(format!("http://127.0.0.1:{}{}", cfg.backend_port, path))
        .send()
        .await
    {
        Ok(r) => (
            r.status().is_success(),
            format!("GET {path} → {}", r.status()),
        ),
        Err(e) => (false, format!("unreachable ({e})")),
    }
}

pub async fn service_statuses(cfg: Option<&ProjectConfig>) -> Vec<ServiceStatus> {
    let mut names: Vec<String> = vec!["nginx".into(), "ufw".into()];
    if let Some(c) = cfg {
        names.insert(0, c.service_name());
    }
    let mut out = Vec::new();
    for n in names {
        let active = run("systemctl", &["is-active", "--quiet", &n])
            .await
            .is_ok();
        out.push(ServiceStatus { name: n, active });
    }
    out
}

pub async fn dir_size(path: &str) -> Option<u64> {
    if path.is_empty() {
        return None;
    }
    let out = run("du", &["-sb", path]).await.ok()?;
    out.split_whitespace().next()?.parse().ok()
}

pub fn disk_usage(path: &str) -> Option<(u64, u64)> {
    use sysinfo::Disks;
    let disks = Disks::new_with_refreshed_list();
    let p = Path::new(if path.is_empty() { "/" } else { path });
    let mut best: Option<(usize, u64, u64)> = None;
    for d in disks.list() {
        let mp = d.mount_point();
        if p.starts_with(mp) {
            let len = mp.as_os_str().len();
            if best.map(|(l, _, _)| len > l).unwrap_or(true) {
                best = Some((len, d.total_space(), d.available_space()));
            }
        }
    }
    let (_, total, avail) = best.or_else(|| {
        disks
            .list()
            .first()
            .map(|d| (0, d.total_space(), d.available_space()))
    })?;
    Some((total, total.saturating_sub(avail)))
}
