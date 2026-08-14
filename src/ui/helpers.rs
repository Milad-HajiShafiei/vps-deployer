//! Small shared rendering helpers.

use ratatui::style::Color;

pub const GREEN: Color = Color::Rgb(80, 220, 120);
pub const YELLOW: Color = Color::Rgb(250, 200, 80);
pub const RED: Color = Color::Rgb(240, 90, 90);
pub const DIM: Color = Color::DarkGray;

pub fn heat(p: f64) -> Color {
    if p >= 85.0 {
        RED
    } else if p >= 60.0 {
        YELLOW
    } else {
        GREEN
    }
}

pub fn human(b: u64) -> String {
    const U: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < 5 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{b} B")
    } else {
        format!("{v:.1} {}", U[i])
    }
}

pub fn bar(ratio: f64, width: usize, ascii: bool) -> String {
    let w = width.max(4);
    let filled = (ratio.clamp(0.0, 1.0) * w as f64).round() as usize;
    let (f_ch, e_ch) = if ascii { ("#", "-") } else { ("█", "░") };
    format!(
        "{}{}",
        f_ch.repeat(filled),
        e_ch.repeat(w.saturating_sub(filled))
    )
}

pub fn spark(data: &[u64], width: usize, ascii: bool) -> String {
    const U: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    const A: [char; 8] = ['_', '.', '-', ':', '=', '+', '*', '#'];
    let glyphs = if ascii { &A } else { &U };
    if width == 0 {
        return String::new();
    }
    let max = data.iter().max().copied().unwrap_or(1).max(1);
    let start = data.len().saturating_sub(width);
    let mut s: String = data[start..]
        .iter()
        .map(|v| glyphs[((*v as f64 / max as f64) * 7.0) as usize])
        .collect();
    while s.chars().count() < width {
        s.insert(0, glyphs[0]);
    }
    s
}
