//! Application state: tabs, form fields, messages.

use crate::config::{self, BackendStack, ProjectConfig};
use crate::theme::{self, Icons};
use ratatui::layout::Rect;
use ratatui_textarea::TextArea;
use std::time::Instant;

pub const HISTORY: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftTab {
    Repos,
    Dirs,
    Ports,
    Nginx,
    Backup,
    Projects,
    Review,
}

impl LeftTab {
    pub const ALL: [LeftTab; 7] = [
        LeftTab::Repos,
        LeftTab::Dirs,
        LeftTab::Ports,
        LeftTab::Nginx,
        LeftTab::Backup,
        LeftTab::Projects,
        LeftTab::Review,
    ];
    pub fn idx(self) -> usize {
        self as usize
    }
    pub fn title(self) -> &'static str {
        match self {
            LeftTab::Repos => "1·Repos",
            LeftTab::Dirs => "2·Dirs",
            LeftTab::Ports => "3·Ports",
            LeftTab::Nginx => "4·Nginx",
            LeftTab::Backup => "5·Backup",
            LeftTab::Projects => "6·Projects",
            LeftTab::Review => "7·Review",
        }
    }
    pub fn next(self) -> LeftTab {
        Self::ALL[(self.idx() + 1) % 7]
    }
    pub fn prev(self) -> LeftTab {
        Self::ALL[(self.idx() + 6) % 7]
    }
}

pub enum FormField {
    Text {
        label: &'static str,
        ta: TextArea<'static>,
    },
    Toggle {
        label: &'static str,
        hint: &'static str,
        value: bool,
    },
    Stack {
        label: &'static str,
        value: BackendStack,
    },
}

pub struct FormTab {
    pub fields: Vec<FormField>,
    pub focus: usize,
}

fn text(label: &'static str, placeholder: &'static str) -> FormField {
    let mut ta = TextArea::default();
    ta.set_placeholder_text(placeholder);
    FormField::Text { label, ta }
}
fn toggle(label: &'static str, hint: &'static str, value: bool) -> FormField {
    FormField::Toggle { label, hint, value }
}

fn fresh_tabs() -> Vec<FormTab> {
    vec![
        // 1 · Repos & stack
        FormTab {
            focus: 0,
            fields: vec![
                text("Project Name *", "my-awesome-app"),
                FormField::Stack {
                    label: "Backend Stack * (◀ ▶ to change)",
                    value: BackendStack::NodeJs,
                },
                text(
                    "Backend Entry Point",
                    "blank = stack default (e.g. server.js)",
                ),
                text(
                    "Backend Repo URL (optional)",
                    "https://github.com/org/backend.git",
                ),
                text(
                    "Frontend Repo URL (optional)",
                    "https://github.com/org/frontend.git",
                ),
                text("Production Branch", "production"),
                text("Systemd Service User", "blank = root"),
            ],
        },
        // 2 · Directories
        FormTab {
            focus: 0,
            fields: vec![
                text("Deploy Directory *", "/var/www/myapp"),
                text("Database Directory", "/var/lib/myapp/db"),
                text("Uploads Directory", "/var/lib/myapp/uploads"),
                text("Backup Directory", "blank = <deploy>/backups"),
            ],
        },
        // 3 · Ports & firewall
        FormTab {
            focus: 0,
            fields: vec![
                text("Backend Port *", "8000"),
                text("Extra UFW Ports (comma separated)", "8080, 9000/tcp"),
                text("API Health Path", "/health"),
                toggle(
                    "Allow backend port in UFW",
                    "opens <port>/tcp in the firewall",
                    true,
                ),
                toggle(
                    "Open HTTP/HTTPS (80, 443)",
                    "required for nginx + certbot",
                    true,
                ),
            ],
        },
        // 4 · Nginx & SSL
        FormTab {
            focus: 0,
            fields: vec![
                text("Domain", "example.com (blank = skip nginx/SSL)"),
                text("Certbot Email", "admin@example.com"),
                text("API Proxy Prefix", "/api/"),
                text("Frontend Dist Directory", "blank = <deploy>/frontend/dist"),
                text("Client Max Body Size", "25m"),
                toggle(
                    "WebSocket proxy support",
                    "adds Upgrade/Connection headers",
                    true,
                ),
            ],
        },
        // 5 · Backup
        FormTab {
            focus: 0,
            fields: vec![
                toggle("Enable backups", "tar.gz snapshots of db + uploads", true),
                text("Backups to Keep (retention)", "10"),
                toggle("Include Database directory", "", true),
                toggle("Include Uploads directory", "", true),
                toggle(
                    "Install daily cron job (03:00)",
                    "runs backup.sh automatically",
                    true,
                ),
            ],
        },
        // 6 · Projects (custom drawn)
        FormTab {
            focus: 0,
            fields: vec![],
        },
        // 7 · Review (custom drawn)
        FormTab {
            focus: 0,
            fields: vec![],
        },
    ]
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    DeployRegen,
    DeployStaged,
    WriteFiles,
    PullDeploy,
    TestApi,
    Restart,
    Delete,
    LoadProject,
    BackupNow,
    DeleteProjectSel,
    NewProject,
    GetDocs,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Click {
    Tab(LeftTab),
    Field { tab: LeftTab, idx: usize },
    Project(usize),
    Button(Action),
}

#[derive(Default, Clone)]
pub struct Metrics {
    pub mem_percent: f64,
    pub mem_used: u64,
    pub mem_total: u64,
    pub cpu_percent: f64,
    pub disk_total: u64,
    pub disk_used: u64,
    pub db_bytes: u64,
    pub uploads_bytes: u64,
    pub rx_rate: u64,
    pub tx_rate: u64,
}

#[derive(Debug, Clone)]
pub struct ServiceStatus {
    pub name: String,
    pub active: bool,
}

pub struct Notification {
    pub message: String,
    pub level: String,
    pub created_at: Instant,
}

pub enum AppMessage {
    Metrics(Metrics),
    Heavy {
        services: Vec<ServiceStatus>,
        db_bytes: u64,
        uploads_bytes: u64,
    },
    ApiHealth {
        ok: bool,
        detail: String,
    },
    Log(String),
    #[allow(dead_code)]
    Notify {
        msg: String,
        level: String,
    },
    DeployDone(Result<String, String>),
    Deleted(Result<String, String>),
    Toast(Result<String, String>),
    GateDone(Result<String, String>),
}

pub struct App {
    pub tabs: Vec<FormTab>,
    pub left_tab: LeftTab,
    pub projects: Vec<ProjectConfig>,
    pub projects_focus: usize,
    pub deployed: Option<ProjectConfig>,
    pub pending_delete: Option<ProjectConfig>,
    pub confirm_delete: Option<Instant>,
    pub metrics: Metrics,
    pub rx_hist: Vec<u64>,
    pub tx_hist: Vec<u64>,
    pub services: Vec<ServiceStatus>,
    pub api_status: Option<(bool, String)>,
    pub logs: Vec<String>,
    pub log_scroll: usize,
    pub notifications: Vec<Notification>,
    pub click_map: Vec<(Rect, Click)>,
    pub hover: Option<Click>,
    pub deploying: bool,
    pub tick: u64,
    pub should_quit: bool,
    pub font_gate: bool,
    pub gate_busy: bool,
    pub ascii: bool,
    pub icons: Icons,
    pub last_key: String,
}

impl App {
    pub fn new() -> Self {
        Self {
            tabs: fresh_tabs(),
            left_tab: LeftTab::Repos,
            projects: crate::config::list_projects(),
            projects_focus: 0,
            deployed: None,
            pending_delete: None,
            confirm_delete: None,
            metrics: Metrics::default(),
            rx_hist: vec![0; HISTORY],
            tx_hist: vec![0; HISTORY],
            services: Vec::new(),
            api_status: None,
            logs: vec!["[INFO] VPS Deployer started — mouse enabled".to_string()],
            log_scroll: 0,
            notifications: Vec::new(),
            click_map: Vec::new(),
            hover: None,
            deploying: false,
            tick: 0,
            should_quit: false,
            font_gate: false,
            gate_busy: false,
            ascii: false,
            icons: theme::UNICODE,
            last_key: "none yet".to_string(),
        }
    }

    pub fn refresh_projects(&mut self) {
        self.projects = crate::config::list_projects();
        if !self.projects.is_empty() {
            self.projects_focus = self.projects_focus.min(self.projects.len() - 1);
        } else {
            self.projects_focus = 0;
        }
    }

    pub fn reset_form(&mut self) {
        self.tabs = fresh_tabs();
        self.deployed = None;
        self.left_tab = LeftTab::Repos;
        self.notify("New project form ready".to_string(), "Info");
    }

    pub fn load_project(&mut self, cfg: ProjectConfig) {
        self.tabs = fresh_tabs();
        self.prefill(&cfg);
        self.deployed = Some(cfg.clone());
        self.left_tab = LeftTab::Review;
        self.notify(
            format!("'{}' loaded — edit anything, then deploy", cfg.project_name),
            "Info",
        );
    }

    pub fn push_log(&mut self, log: String) {
        self.logs.push(log);
        if self.logs.len() > 1000 {
            self.logs.remove(0);
        }
    }

    pub fn notify(&mut self, msg: String, level: &str) {
        self.notifications.push(Notification {
            message: msg,
            level: level.to_string(),
            created_at: Instant::now(),
        });
        if self.notifications.len() > 6 {
            self.notifications.remove(0);
        }
    }

    fn text_of(&self, tab: usize, idx: usize) -> String {
        self.tabs
            .get(tab)
            .and_then(|t| t.fields.get(idx))
            .and_then(|f| match f {
                FormField::Text { ta, .. } => Some(ta.lines().join("\n").trim().to_string()),
                _ => None,
            })
            .unwrap_or_default()
    }

    fn toggle_of(&self, tab: usize, idx: usize) -> bool {
        matches!(
            self.tabs.get(tab).and_then(|t| t.fields.get(idx)),
            Some(FormField::Toggle { value: true, .. })
        )
    }

    pub fn set_text(&mut self, tab: usize, idx: usize, s: &str) {
        if s.is_empty() {
            return;
        }
        if let FormField::Text { ta, .. } = &mut self.tabs[tab].fields[idx] {
            ta.insert_str(s.to_string());
        }
    }

    pub fn collect_config(&self) -> ProjectConfig {
        let mut cfg = ProjectConfig::default();
        cfg.project_name = self.text_of(0, 0);
        cfg.stack = self
            .tabs
            .get(0)
            .and_then(|t| t.fields.get(1))
            .and_then(|f| match f {
                FormField::Stack { value, .. } => Some(*value),
                _ => None,
            })
            .unwrap_or(config::BackendStack::NodeJs);
        cfg.entry_point = self.text_of(0, 2);
        cfg.backend_repo = self.text_of(0, 3);
        cfg.frontend_repo = self.text_of(0, 4);
        cfg.branch = self.text_of(0, 5);
        cfg.service_user = self.text_of(0, 6);

        cfg.target_dir = self.text_of(1, 0);
        cfg.db_dir = self.text_of(1, 1);
        cfg.uploads_dir = self.text_of(1, 2);
        cfg.backup_dir = self.text_of(1, 3);

        cfg.backend_port = self.text_of(2, 0);
        cfg.extra_ports = self.text_of(2, 1);
        cfg.health_path = self.text_of(2, 2);
        cfg.ufw_backend = self.toggle_of(2, 3);
        cfg.ufw_web = self.toggle_of(2, 4);

        cfg.domain = self.text_of(3, 0);
        cfg.cert_email = self.text_of(3, 1);
        cfg.api_prefix = self.text_of(3, 2);
        cfg.frontend_dist = self.text_of(3, 3);
        cfg.max_body_size = self.text_of(3, 4);
        cfg.websocket = self.toggle_of(3, 5);

        cfg.backup_enabled = self.toggle_of(4, 0);
        cfg.backup_retention = self.text_of(4, 1);
        cfg.backup_db = self.toggle_of(4, 2);
        cfg.backup_uploads = self.toggle_of(4, 3);
        cfg.backup_cron = self.toggle_of(4, 4);

        // defaults
        if cfg.branch.is_empty() {
            cfg.branch = "production".into();
        }
        if cfg.entry_point.is_empty() {
            cfg.entry_point = cfg.stack.default_entry().into();
        }
        if cfg.health_path.is_empty() {
            cfg.health_path = "/health".into();
        }
        if cfg.api_prefix.is_empty() {
            cfg.api_prefix = "/api/".into();
        }
        if cfg.max_body_size.is_empty() {
            cfg.max_body_size = "25m".into();
        }
        if cfg.backup_retention.is_empty() {
            cfg.backup_retention = "10".into();
        }
        if cfg.backup_dir.is_empty() && !cfg.target_dir.is_empty() {
            cfg.backup_dir = format!("{}/backups", cfg.target_dir);
        }
        if cfg.frontend_dist.is_empty() && !cfg.target_dir.is_empty() {
            cfg.frontend_dist = format!("{}/frontend/dist", cfg.target_dir);
        }
        cfg
    }

    pub fn prefill(&mut self, cfg: &ProjectConfig) {
        self.set_text(0, 0, &cfg.project_name);
        if let FormField::Stack { value, .. } = &mut self.tabs[0].fields[1] {
            *value = cfg.stack;
        }
        self.set_text(0, 2, &cfg.entry_point);
        self.set_text(0, 3, &cfg.backend_repo);
        self.set_text(0, 4, &cfg.frontend_repo);
        self.set_text(0, 5, &cfg.branch);
        self.set_text(0, 6, &cfg.service_user);
        self.set_text(1, 0, &cfg.target_dir);
        self.set_text(1, 1, &cfg.db_dir);
        self.set_text(1, 2, &cfg.uploads_dir);
        self.set_text(1, 3, &cfg.backup_dir);
        self.set_text(2, 0, &cfg.backend_port);
        self.set_text(2, 1, &cfg.extra_ports);
        self.set_text(2, 2, &cfg.health_path);
        if let FormField::Toggle { value, .. } = &mut self.tabs[2].fields[3] {
            *value = cfg.ufw_backend;
        }
        if let FormField::Toggle { value, .. } = &mut self.tabs[2].fields[4] {
            *value = cfg.ufw_web;
        }
        self.set_text(3, 0, &cfg.domain);
        self.set_text(3, 1, &cfg.cert_email);
        self.set_text(3, 2, &cfg.api_prefix);
        self.set_text(3, 3, &cfg.frontend_dist);
        self.set_text(3, 4, &cfg.max_body_size);
        if let FormField::Toggle { value, .. } = &mut self.tabs[3].fields[5] {
            *value = cfg.websocket;
        }
        if let FormField::Toggle { value, .. } = &mut self.tabs[4].fields[0] {
            *value = cfg.backup_enabled;
        }
        self.set_text(4, 1, &cfg.backup_retention);
        if let FormField::Toggle { value, .. } = &mut self.tabs[4].fields[2] {
            *value = cfg.backup_db;
        }
        if let FormField::Toggle { value, .. } = &mut self.tabs[4].fields[3] {
            *value = cfg.backup_uploads;
        }
        if let FormField::Toggle { value, .. } = &mut self.tabs[4].fields[4] {
            *value = cfg.backup_cron;
        }
    }
}
