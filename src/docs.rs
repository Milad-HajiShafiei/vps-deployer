//! 📖 Documentation generation — "Get docs" button.
//! Renders templates/documentation.md.tpl with the project's real values
//! and writes it to <target_dir>/.vps-deployer/DOCUMENTATION.md

use crate::actions;
use crate::config::ProjectConfig;
use crate::templates::render;

const DOC_TPL: &str = include_str!("../templates/documentation.md.tpl");

pub fn generate(cfg: &ProjectConfig, date: &str) -> String {
    let staging = cfg.staging_dir();
    let svc = cfg.service_name();
    let user = if cfg.service_user.trim().is_empty() {
        "root".to_string()
    } else {
        cfg.service_user.trim().to_string()
    };

    let domain_disp = if cfg.domain.is_empty() {
        "— (nginx/SSL skipped)".to_string()
    } else {
        cfg.domain.clone()
    };
    let domain_file = if cfg.domain.is_empty() {
        "nginx-<domain>.conf (skipped)".to_string()
    } else {
        format!("nginx-{}.conf", cfg.domain)
    };
    let nginx_installed = if cfg.domain.is_empty() {
        "— (no domain configured)".to_string()
    } else {
        format!(
            "`/etc/nginx/sites-available/{}` (symlinked into sites-enabled)",
            cfg.domain
        )
    };

    let be_repo = if cfg.backend_repo.is_empty() {
        "not configured".to_string()
    } else {
        cfg.backend_repo.clone()
    };
    let fe_repo = if cfg.frontend_repo.is_empty() {
        "not configured".to_string()
    } else {
        cfg.frontend_repo.clone()
    };

    let cron_line = if cfg.backup_enabled && cfg.backup_cron {
        format!(
            "`0 3 * * * {staging}/backup.sh >> {}/cron.log 2>&1`",
            cfg.backup_dir
        )
    } else {
        "not installed".to_string()
    };

    let ufw_rules = {
        let mut v: Vec<String> = Vec::new();
        if cfg.ufw_web {
            v.push("80/tcp · 443/tcp".into());
        }
        if cfg.ufw_backend {
            v.push(format!("{}/tcp (backend)", cfg.backend_port));
        }
        for p in cfg.extra_ports.split(',') {
            let p = p.trim();
            if !p.is_empty() {
                v.push(p.to_string());
            }
        }
        if v.is_empty() {
            "none".to_string()
        } else {
            v.join(" · ")
        }
    };

    let health = if cfg.health_path.starts_with('/') {
        cfg.health_path.clone()
    } else {
        format!("/{}", cfg.health_path)
    };

    render(
        DOC_TPL,
        &[
            ("PROJECT", &cfg.project_name),
            ("DATE", date),
            ("STACK", cfg.stack.label()),
            ("BRANCH", &cfg.branch),
            ("PORT", &cfg.backend_port),
            ("DOMAIN", &domain_disp),
            ("DOMAIN_FILE", &domain_file),
            ("NGINX_INSTALLED", &nginx_installed),
            ("HEALTH", &health),
            ("USER", &user),
            ("TARGET_DIR", &cfg.target_dir),
            ("BE_REPO", &be_repo),
            ("FE_REPO", &fe_repo),
            ("SERVICE", &svc),
            ("STAGING", &staging),
            ("DB_DIR", &cfg.db_dir),
            ("UPLOADS_DIR", &cfg.uploads_dir),
            ("BACKUP_DIR", &cfg.backup_dir),
            (
                "EXEC_START",
                &cfg.stack
                    .exec_start(&format!("{}/backend", cfg.target_dir), &cfg.entry_point),
            ),
            ("DIST", &cfg.frontend_dist),
            ("PREFIX", &cfg.api_prefix),
            ("MAX_BODY", &cfg.max_body_size),
            ("UFW_RULES", &ufw_rules),
            ("BUILD_CMD", cfg.stack.build_command()),
            ("CRON_LINE", &cron_line),
            ("RETENTION", &cfg.backup_retention),
        ],
    )
}

/// Writes DOCUMENTATION.md into the project's staging dir, returns the path.
pub async fn write_docs(cfg: &ProjectConfig) -> Result<String, String> {
    let date = actions::run("date", &["+%Y-%m-%d %H:%M:%S %Z"])
        .await
        .unwrap_or_else(|_| "unknown date".into());
    let staging = cfg.staging_dir();
    tokio::fs::create_dir_all(&staging)
        .await
        .map_err(|e| format!("create {staging}: {e}"))?;
    let path = format!("{staging}/DOCUMENTATION.md");
    tokio::fs::write(&path, generate(cfg, &date))
        .await
        .map_err(|e| format!("write docs: {e}"))?;
    Ok(path)
}
