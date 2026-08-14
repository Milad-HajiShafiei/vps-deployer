//! Project configuration model + multi-project persistence.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendStack {
    NodeJs,
    TypeScript,
    Rust,
    Go,
    Python,
}

impl BackendStack {
    pub const ALL: [BackendStack; 5] = [
        BackendStack::NodeJs,
        BackendStack::TypeScript,
        BackendStack::Rust,
        BackendStack::Go,
        BackendStack::Python,
    ];
    pub fn label(self) -> &'static str {
        match self {
            BackendStack::NodeJs => "Node.js (JavaScript)",
            BackendStack::TypeScript => "Node.js (TypeScript)",
            BackendStack::Rust => "Rust (cargo)",
            BackendStack::Go => "Go (go build)",
            BackendStack::Python => "Python 3",
        }
    }
    pub fn short(self) -> &'static str {
        match self {
            BackendStack::NodeJs => "node",
            BackendStack::TypeScript => "ts",
            BackendStack::Rust => "rust",
            BackendStack::Go => "go",
            BackendStack::Python => "py",
        }
    }
    pub fn next(self) -> BackendStack {
        let i = Self::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Self::ALL[(i + 1) % 5]
    }
    pub fn prev(self) -> BackendStack {
        let i = Self::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Self::ALL[(i + 4) % 5]
    }
    /// Build command — written into deploy.sh and run on first deploy.
    pub fn build_command(self) -> &'static str {
        match self {
            BackendStack::NodeJs => "npm ci --omit=dev || npm install --omit=dev",
            BackendStack::TypeScript => "npm ci && npm run build",
            BackendStack::Rust => "cargo build --release",
            BackendStack::Go => "go build -o app .",
            BackendStack::Python => "python3 -m pip install -r requirements.txt",
        }
    }
    pub fn default_entry(self) -> &'static str {
        match self {
            BackendStack::NodeJs => "server.js",
            BackendStack::TypeScript => "main.js",
            BackendStack::Rust => "app",
            BackendStack::Go => "app",
            BackendStack::Python => "main.py",
        }
    }
    /// ExecStart for the systemd unit.
    pub fn exec_start(self, dir: &str, entry: &str) -> String {
        let e = if entry.trim().is_empty() {
            self.default_entry()
        } else {
            entry.trim()
        };
        match self {
            BackendStack::NodeJs => format!("/usr/bin/env node {dir}/{e}"),
            BackendStack::TypeScript => format!("/usr/bin/env node {dir}/dist/{e}"),
            BackendStack::Rust => format!("{dir}/target/release/{e}"),
            BackendStack::Go => format!("{dir}/{e}"),
            BackendStack::Python => format!("/usr/bin/env python3 {dir}/{e}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectConfig {
    // repos & stack
    pub project_name: String,
    pub stack: BackendStack,
    pub entry_point: String,
    pub backend_repo: String,
    pub frontend_repo: String,
    pub branch: String,
    pub service_user: String,
    // directories
    pub target_dir: String,
    pub db_dir: String,
    pub uploads_dir: String,
    pub backup_dir: String,
    // ports & firewall
    pub backend_port: String,
    pub extra_ports: String,
    pub health_path: String,
    pub ufw_backend: bool,
    pub ufw_web: bool,
    // nginx
    pub domain: String,
    pub cert_email: String,
    pub api_prefix: String,
    pub frontend_dist: String,
    pub max_body_size: String,
    pub websocket: bool,
    // backup
    pub backup_enabled: bool,
    pub backup_retention: String,
    pub backup_db: bool,
    pub backup_uploads: bool,
    pub backup_cron: bool,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            project_name: String::new(),
            stack: BackendStack::NodeJs,
            entry_point: String::new(),
            backend_repo: String::new(),
            frontend_repo: String::new(),
            branch: "production".into(),
            service_user: String::new(),
            target_dir: String::new(),
            db_dir: String::new(),
            uploads_dir: String::new(),
            backup_dir: String::new(),
            backend_port: String::new(),
            extra_ports: String::new(),
            health_path: "/health".into(),
            ufw_backend: true,
            ufw_web: true,
            domain: String::new(),
            cert_email: String::new(),
            api_prefix: "/api/".into(),
            frontend_dist: String::new(),
            max_body_size: "25m".into(),
            websocket: true,
            backup_enabled: true,
            backup_retention: "10".into(),
            backup_db: true,
            backup_uploads: true,
            backup_cron: true,
        }
    }
}

impl ProjectConfig {
    pub fn service_name(&self) -> String {
        format!("{}-backend", self.project_name)
    }
    pub fn staging_dir(&self) -> String {
        format!("{}/.vps-deployer", self.target_dir)
    }
    pub fn retention_u32(&self) -> u32 {
        self.backup_retention.trim().parse().unwrap_or(10).max(1)
    }
}

pub fn validate(cfg: &ProjectConfig) -> Result<(), String> {
    if cfg.project_name.trim().is_empty() {
        return Err("Project Name is required".into());
    }
    if cfg.target_dir.trim().is_empty() {
        return Err("Deploy Directory is required".into());
    }
    if cfg.backend_port.parse::<u16>().is_err() {
        return Err("Backend Port must be a valid number".into());
    }
    if cfg.backend_repo.is_empty() && cfg.frontend_repo.is_empty() {
        return Err("Provide at least one repository (backend or frontend)".into());
    }
    Ok(())
}

// ── persistence: one JSON file per project ─────────────────────────
pub fn projects_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("vps-deployer").join("projects"))
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn save_project(cfg: &ProjectConfig) -> Result<PathBuf, String> {
    let dir = projects_dir().ok_or("cannot resolve config dir")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.json", sanitize(&cfg.project_name)));
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn list_projects() -> Vec<ProjectConfig> {
    let Some(dir) = projects_dir() else {
        return Vec::new();
    };
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().map(|x| x == "json").unwrap_or(false) {
            if let Ok(s) = std::fs::read_to_string(&p) {
                if let Ok(cfg) = serde_json::from_str::<ProjectConfig>(&s) {
                    out.push(cfg);
                }
            }
        }
    }
    out.sort_by(|a, b| a.project_name.cmp(&b.project_name));
    out
}

pub fn remove_project_file(name: &str) {
    if let Some(dir) = projects_dir() {
        let _ = std::fs::remove_file(dir.join(format!("{}.json", sanitize(name))));
    }
}
