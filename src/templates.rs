//! Renders the editable template files (deploy.sh, systemd, nginx, backup).

use crate::config::ProjectConfig;

const DEPLOY_TPL: &str = include_str!("../templates/deploy.sh.tpl");
const SERVICE_TPL: &str = include_str!("../templates/backend.service.tpl");
const NGINX_TPL: &str = include_str!("../templates/nginx.conf.tpl");
const BACKUP_TPL: &str = include_str!("../templates/backup.sh.tpl");

pub fn render(tpl: &str, vars: &[(&str, &str)]) -> String {
    let mut out = tpl.to_string();
    // longest keys first so e.g. {{DOMAIN_FILE}} isn't broken by {{DOMAIN}}
    let mut sorted: Vec<&(&str, &str)> = vars.iter().collect();
    sorted.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    for (k, v) in sorted {
        out = out.replace(&format!("{{{{{k}}}}}"), v);
    }
    out
}

pub fn deploy_sh(cfg: &ProjectConfig) -> String {
    let backend_block = if cfg.backend_repo.is_empty() {
        "echo '==> no backend repo configured, skipping'".to_string()
    } else {
        format!(
            r#"if [ -d "$BACKEND_DIR/.git" ]; then
  cd "$BACKEND_DIR"
  git fetch --all --prune
  git checkout "$BRANCH"
  git pull origin "$BRANCH"
  echo "==> building backend ({stack}) ..."
  {build}
  sudo systemctl restart "$SERVICE" || true
else
  echo "!! backend not cloned yet, skipping"
fi"#,
            stack = cfg.stack.label(),
            build = cfg.stack.build_command(),
        )
    };

    let frontend_block = if cfg.frontend_repo.is_empty() {
        "echo '==> no frontend repo configured, skipping'".to_string()
    } else {
        r#"if [ -d "$FRONTEND_DIR/.git" ]; then
  cd "$FRONTEND_DIR"
  git fetch --all --prune
  git checkout "$BRANCH"
  git pull origin "$BRANCH"
  if [ -f package.json ]; then
    echo "==> building frontend ..."
    npm ci
    npm run build --if-present
  fi
else
  echo "!! frontend not cloned yet, skipping"
fi"#
        .to_string()
    };

    render(
        DEPLOY_TPL,
        &[
            ("PROJECT", &cfg.project_name),
            ("BRANCH", &cfg.branch),
            ("ROOT", &cfg.target_dir),
            ("SERVICE", &cfg.service_name()),
            ("BACKEND_BLOCK", &backend_block),
            ("FRONTEND_BLOCK", &frontend_block),
        ],
    )
}

pub fn backend_service(cfg: &ProjectConfig) -> String {
    let be_dir = format!("{}/backend", cfg.target_dir);
    let user = if cfg.service_user.trim().is_empty() {
        "root".to_string()
    } else {
        cfg.service_user.trim().to_string()
    };
    render(
        SERVICE_TPL,
        &[
            ("PROJECT", &cfg.project_name),
            ("STACK", cfg.stack.short()),
            ("USER", &user),
            ("BACKEND_DIR", &be_dir),
            ("PORT", &cfg.backend_port),
            ("DB_DIR", &cfg.db_dir),
            ("UPLOADS_DIR", &cfg.uploads_dir),
            (
                "EXEC_START",
                &cfg.stack.exec_start(&be_dir, &cfg.entry_point),
            ),
        ],
    )
}

pub fn nginx_conf(cfg: &ProjectConfig) -> String {
    let ws = if cfg.websocket {
        "        proxy_set_header Upgrade $http_upgrade;\n        proxy_set_header Connection \"upgrade\";"
    } else {
        ""
    };
    render(
        NGINX_TPL,
        &[
            ("PROJECT", &cfg.project_name),
            ("DOMAIN", &cfg.domain),
            ("DIST", &cfg.frontend_dist),
            ("MAX_BODY", &cfg.max_body_size),
            ("PREFIX", &cfg.api_prefix),
            ("PORT", &cfg.backend_port),
            ("WEBSOCKET_BLOCK", ws),
        ],
    )
}

pub fn backup_sh(cfg: &ProjectConfig) -> String {
    let mut sections = String::new();
    if cfg.backup_db && !cfg.db_dir.is_empty() {
        sections.push_str(&format!(
            "if [ -d \"{db}\" ]; then\n  echo \"==> backing up database directory\"\n  mkdir -p \"$TMP/db\"\n  cp -a \"{db}/.\" \"$TMP/db/\"\nfi\n",
            db = cfg.db_dir
        ));
    }
    if cfg.backup_uploads && !cfg.uploads_dir.is_empty() {
        sections.push_str(&format!(
            "if [ -d \"{up}\" ]; then\n  echo \"==> backing up uploads directory\"\n  mkdir -p \"$TMP/uploads\"\n  cp -a \"{up}/.\" \"$TMP/uploads/\"\nfi\n",
            up = cfg.uploads_dir
        ));
    }
    if sections.is_empty() {
        sections = "echo 'nothing selected for backup'".to_string();
    }
    let retention = cfg.retention_u32();
    render(
        BACKUP_TPL,
        &[
            ("PROJECT", &cfg.project_name),
            ("BACKUP_DIR", &cfg.backup_dir),
            ("SECTIONS", &sections),
            ("RETENTION", &retention.to_string()),
            ("RETENTION_PLUS_1", &(retention + 1).to_string()),
        ],
    )
}
