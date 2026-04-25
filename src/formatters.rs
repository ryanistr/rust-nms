use ratatui::style::Color;

pub fn fmt_bytes(b: u64) -> String {
    const GIB: u64 = 1 << 30;
    const MIB: u64 = 1 << 20;
    const KIB: u64 = 1 << 10;
    if b >= GIB {
        format!("{:.2} GiB", b as f64 / GIB as f64)
    } else if b >= MIB {
        format!("{:.2} MiB", b as f64 / MIB as f64)
    } else if b >= KIB {
        format!("{:.2} KiB", b as f64 / KIB as f64)
    } else {
        format!("{} B", b)
    }
}

pub fn fmt_rate(bytes_per_sec: u64) -> String {
    let bits = bytes_per_sec * 8;
    if bits >= 1_000_000_000 {
        format!("{:.2} Gbps", bits as f64 / 1e9)
    } else if bits >= 1_000_000 {
        format!("{:.2} Mbps", bits as f64 / 1e6)
    } else if bits >= 1_000 {
        format!("{:.2} Kbps", bits as f64 / 1e3)
    } else {
        format!("{} bps", bits)
    }
}

pub fn fmt_speed(speed: &str) -> String {
    match speed.trim() {
        s if s.is_empty() || s == "-1" || s == "unknown" => "—".to_string(),
        s => format!("{} Mbps", s),
    }
}

pub fn gauge_color(pct: u16) -> Color {
    if pct >= 80 {
        Color::Red
    } else if pct >= 50 {
        Color::Yellow
    } else {
        Color::Green
    }
}