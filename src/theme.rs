//! Colors, icon sets and terminal capability detection.

#[derive(Clone, Copy)]
pub struct Icons {
    pub rocket: &'static str,
    pub shield: &'static str,
    pub brain: &'static str,
    pub cpu: &'static str,
    pub db: &'static str,
    pub r#box: &'static str,
    pub disk: &'static str,
    pub down: &'static str,
    pub up: &'static str,
    #[allow(dead_code)]
    pub logs: &'static str,
    pub trash: &'static str,
    pub backup: &'static str,
    pub bolt: &'static str,
    #[allow(dead_code)]
    pub folder: &'static str,
    pub plus: &'static str,
    pub enter: &'static str,
    pub dot_ok: &'static str,
    pub dot_bad: &'static str,
}

pub const UNICODE: Icons = Icons {
    rocket: "🚀",
    shield: "🛡 ",
    brain: "🧠",
    cpu: "⚙",
    db: "🗄 ",
    r#box: "📦",
    disk: "💽",
    down: "▼",
    up: "▲",
    logs: "📜",
    trash: "🗑 ",
    backup: "💾",
    bolt: "⚡",
    folder: "📂",
    plus: "✚",
    enter: "↵",
    dot_ok: "●",
    dot_bad: "●",
};

pub const ASCII: Icons = Icons {
    rocket: ">>",
    shield: "SV",
    brain: "RAM",
    cpu: "CPU",
    db: "DB",
    r#box: "PKG",
    disk: "DSK",
    down: "v",
    up: "^",
    logs: "LOG",
    trash: "DEL",
    backup: "BAK",
    bolt: "API",
    folder: "DIR",
    plus: "+",
    enter: ">",
    dot_ok: "+",
    dot_bad: "x",
};

/// Heuristic: does the current locale look UTF-8 capable?
pub fn looks_utf8() -> bool {
    for var in ["LC_ALL", "LC_CTYPE", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            let v = v.to_uppercase();
            if v.contains("UTF-8") || v.contains("UTF8") {
                return true;
            }
        }
    }
    false
}
