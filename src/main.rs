use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph, Sparkline, Tabs, Wrap},
    Terminal,
};
use std::{
    collections::HashMap,
    error::Error,
    fs, io,
    process::Command,
    time::{Duration, Instant},
};

// ─── Constants ───────────────────────────────────────────────────────────────

const HISTORY_SIZE: usize = 60;

// ─── Data Structures ─────────────────────────────────────────────────────────

#[derive(Clone, Default)]
struct Interface {
    name: String,
    operstate: String,
    mac: String,
    mtu: u64,
    speed: String,
    rx_bytes: u64,
    tx_bytes: u64,
    rx_packets: u64,
    tx_packets: u64,
    rx_errors: u64,
    tx_errors: u64,
    rx_dropped: u64,
    tx_dropped: u64,
    ipv4: Vec<String>,
    ipv6: Vec<String>,
}

struct TrafficHistory {
    rx_history: Vec<u64>, // bytes/sec samples
    tx_history: Vec<u64>,
    prev_rx: u64,
    prev_tx: u64,
}

impl TrafficHistory {
    fn new(rx: u64, tx: u64) -> Self {
        Self {
            rx_history: vec![0u64; HISTORY_SIZE],
            tx_history: vec![0u64; HISTORY_SIZE],
            prev_rx: rx,
            prev_tx: tx,
        }
    }

    fn update(&mut self, rx: u64, tx: u64) {
        let rx_rate = rx.saturating_sub(self.prev_rx);
        let tx_rate = tx.saturating_sub(self.prev_tx);
        self.rx_history.push(rx_rate);
        self.tx_history.push(tx_rate);
        if self.rx_history.len() > HISTORY_SIZE { self.rx_history.remove(0); }
        if self.tx_history.len() > HISTORY_SIZE { self.tx_history.remove(0); }
        self.prev_rx = rx;
        self.prev_tx = tx;
    }

    fn current_rx(&self) -> u64 { self.rx_history.last().copied().unwrap_or(0) }
    fn current_tx(&self) -> u64 { self.tx_history.last().copied().unwrap_or(0) }
    fn peak_rx(&self) -> u64 { self.rx_history.iter().copied().max().unwrap_or(0) }
    fn peak_tx(&self) -> u64 { self.tx_history.iter().copied().max().unwrap_or(0) }
}

// ─── System Data Readers ─────────────────────────────────────────────────────

fn read_file(path: &std::path::Path, name: &str) -> String {
    fs::read_to_string(path.join(name))
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn read_stat_u64(path: &std::path::Path, name: &str) -> u64 {
    read_file(path, name).parse().unwrap_or(0)
}

fn get_ip_addresses(iface: &str) -> (Vec<String>, Vec<String>) {
    let Ok(out) = Command::new("ip").args(["addr", "show", iface]).output() else {
        return (vec![], vec![]);
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut v4 = vec![];
    let mut v6 = vec![];
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("inet6 ") {
            if let Some(a) = t.split_whitespace().nth(1) { v6.push(a.to_string()); }
        } else if t.starts_with("inet ") {
            if let Some(a) = t.split_whitespace().nth(1) { v4.push(a.to_string()); }
        }
    }
    (v4, v6)
}

fn get_interfaces() -> Vec<Interface> {
    let Ok(entries) = fs::read_dir("/sys/class/net") else { return vec![]; };
    let mut list: Vec<Interface> = entries
        .flatten()
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let p = entry.path();
            let s = p.join("statistics");
            let (ipv4, ipv6) = get_ip_addresses(&name);
            Interface {
                operstate: read_file(&p, "operstate"),
                mac:       read_file(&p, "address"),
                mtu:       read_file(&p, "mtu").parse().unwrap_or(0),
                speed:     read_file(&p, "speed"),
                rx_bytes:   read_stat_u64(&s, "rx_bytes"),
                tx_bytes:   read_stat_u64(&s, "tx_bytes"),
                rx_packets: read_stat_u64(&s, "rx_packets"),
                tx_packets: read_stat_u64(&s, "tx_packets"),
                rx_errors:  read_stat_u64(&s, "rx_errors"),
                tx_errors:  read_stat_u64(&s, "tx_errors"),
                rx_dropped: read_stat_u64(&s, "rx_dropped"),
                tx_dropped: read_stat_u64(&s, "tx_dropped"),
                name, ipv4, ipv6,
            }
        })
        .collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

fn get_hostname() -> String {
    fs::read_to_string("/etc/hostname").unwrap_or_default().trim().to_string()
}

fn get_uptime_str() -> String {
    let raw = fs::read_to_string("/proc/uptime").unwrap_or_default();
    let secs = raw.split_whitespace().next()
        .and_then(|x| x.parse::<f64>().ok())
        .unwrap_or(0.0) as u64;
    let (d, h, m, s) = (secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60, secs % 60);
    if d > 0 { format!("{}d {}h {}m", d, h, m) }
    else if h > 0 { format!("{}h {}m {:02}s", h, m, s) }
    else { format!("{}m {:02}s", m, s) }
}

// ─── Formatters ──────────────────────────────────────────────────────────────

fn fmt_bytes(b: u64) -> String {
    const GIB: u64 = 1 << 30;
    const MIB: u64 = 1 << 20;
    const KIB: u64 = 1 << 10;
    if b >= GIB      { format!("{:.2} GiB", b as f64 / GIB as f64) }
    else if b >= MIB { format!("{:.2} MiB", b as f64 / MIB as f64) }
    else if b >= KIB { format!("{:.2} KiB", b as f64 / KIB as f64) }
    else             { format!("{} B", b) }
}

fn fmt_rate(bytes_per_sec: u64) -> String {
    let bits = bytes_per_sec * 8;
    if bits >= 1_000_000_000      { format!("{:.2} Gbps", bits as f64 / 1e9) }
    else if bits >= 1_000_000     { format!("{:.2} Mbps", bits as f64 / 1e6) }
    else if bits >= 1_000         { format!("{:.2} Kbps", bits as f64 / 1e3) }
    else                          { format!("{} bps", bits) }
}

fn fmt_speed(speed: &str) -> String {
    match speed.trim() {
        s if s.is_empty() || s == "-1" || s == "unknown" => "—".to_string(),
        s => format!("{} Mbps", s),
    }
}

fn gauge_color(pct: u16) -> Color {
    if pct >= 80 { Color::Red }
    else if pct >= 50 { Color::Yellow }
    else { Color::Green }
}

// ─── Tab enum ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Tab { Details, Traffic, Packets }

impl Tab {
    fn next(self) -> Self {
        match self {
            Self::Details => Self::Traffic,
            Self::Traffic => Self::Packets,
            Self::Packets => Self::Details,
        }
    }
    fn prev(self) -> Self {
        match self {
            Self::Details => Self::Packets,
            Self::Traffic => Self::Details,
            Self::Packets => Self::Traffic,
        }
    }
    fn index(self) -> usize {
        match self { Self::Details => 0, Self::Traffic => 1, Self::Packets => 2 }
    }
}

// ─── App State ───────────────────────────────────────────────────────────────

struct App {
    interfaces: Vec<Interface>,
    list_state: ListState,
    history: HashMap<String, TrafficHistory>,
    tab: Tab,
    show_all: bool,    // true = all, false = UP only
    refresh_count: u64,
}

impl App {
    fn new() -> Self {
        let ifaces = get_interfaces();
        let mut list_state = ListState::default();
        if !ifaces.is_empty() { list_state.select(Some(0)); }
        let mut history = HashMap::new();
        for i in &ifaces {
            history.insert(i.name.clone(), TrafficHistory::new(i.rx_bytes, i.tx_bytes));
        }
        Self {
            interfaces: ifaces, list_state, history,
            tab: Tab::Details, show_all: true, refresh_count: 0,
        }
    }

    // Returns indices into self.interfaces that pass the current filter
    fn filtered_indices(&self) -> Vec<usize> {
        self.interfaces.iter().enumerate()
            .filter(|(_, i)| self.show_all || i.operstate == "up")
            .map(|(idx, _)| idx)
            .collect()
    }

    fn selected_iface(&self) -> Option<&Interface> {
        let f = self.filtered_indices();
        self.list_state.selected()
            .and_then(|s| f.get(s))
            .and_then(|&idx| self.interfaces.get(idx))
    }

    fn navigate(&mut self, delta: i64) {
        let n = self.filtered_indices().len();
        if n == 0 { return; }
        let cur = self.list_state.selected().unwrap_or(0) as i64;
        let next = ((cur + delta).rem_euclid(n as i64)) as usize;
        self.list_state.select(Some(next));
    }

    fn refresh(&mut self) {
        self.interfaces = get_interfaces();
        for i in &self.interfaces {
            let h = self.history.entry(i.name.clone())
                .or_insert_with(|| TrafficHistory::new(i.rx_bytes, i.tx_bytes));
            h.update(i.rx_bytes, i.tx_bytes);
        }
        let n = self.filtered_indices().len();
        if let Some(s) = self.list_state.selected() {
            if n == 0 { self.list_state.select(None); }
            else if s >= n { self.list_state.select(Some(n - 1)); }
        }
        self.refresh_count += 1;
    }

    fn stats_summary(&self) -> (usize, usize) {
        let up = self.interfaces.iter().filter(|i| i.operstate == "up").count();
        (up, self.interfaces.len())
    }
}

// ─── Entry Point ─────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let app = App::new();
    let res = run_app(&mut terminal, app);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    if let Err(e) = res { eprintln!("{e:?}"); }
    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    let tick = Duration::from_millis(1000);
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|f| ui(f, &mut app))?;
        let timeout = tick.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(k) = event::read()? {
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Down  | KeyCode::Char('j') => app.navigate(1),
                    KeyCode::Up    | KeyCode::Char('k') => app.navigate(-1),
                    KeyCode::Tab                        => app.tab = app.tab.next(),
                    KeyCode::BackTab                    => app.tab = app.tab.prev(),
                    KeyCode::Char('1')                  => app.tab = Tab::Details,
                    KeyCode::Char('2')                  => app.tab = Tab::Traffic,
                    KeyCode::Char('3')                  => app.tab = Tab::Packets,
                    KeyCode::Char('f')                  => {
                        app.show_all = !app.show_all;
                        app.list_state.select(Some(0));
                    }
                    KeyCode::Char('r')                  => app.refresh(),
                    _ => {}
                }
            }
        }
        if last_tick.elapsed() >= tick {
            app.refresh();
            last_tick = Instant::now();
        }
    }
}

// ─── Top-level UI ────────────────────────────────────────────────────────────

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    let area = f.size();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Min(0),     // body
            Constraint::Length(3),  // footer
        ])
        .split(area);

    render_header(f, app, rows[0]);
    render_body(f, app, rows[1]);
    render_footer(f, app, rows[2]);
}

// ─── Header ──────────────────────────────────────────────────────────────────

fn render_header(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let (up, total) = app.stats_summary();
    let hostname = get_hostname();
    let uptime = get_uptime_str();
    let status_indicator = if up == total { "◆ ALL UP" } else { "◈ DEGRADED" };
    let status_color = if up == total { Color::Green } else { Color::Yellow };

    let line = Line::from(vec![
        Span::styled(" ◈ NMS ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" {} ", hostname), Style::default().fg(Color::White)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" ⏱ {} ", uptime), Style::default().fg(Color::White)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" Interfaces: {}/{} UP ", up, total), Style::default().fg(Color::White)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" {} ", status_indicator), Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
    ]);

    f.render_widget(
        Paragraph::new(line)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))),
        area,
    );
}

// ─── Footer ──────────────────────────────────────────────────────────────────

fn render_footer(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let filter_lbl = if app.show_all { "All" } else { "UP" };
    let refresh_lbl = format!("#{}", app.refresh_count);

    let line = Line::from(vec![
        Span::styled(" ↑↓/jk ", Style::default().fg(Color::Cyan)),
        Span::styled("Navigate  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Tab/1-3 ", Style::default().fg(Color::Cyan)),
        Span::styled("Switch View  ", Style::default().fg(Color::DarkGray)),
        Span::styled("f ", Style::default().fg(Color::Cyan)),
        Span::styled(format!("Filter [{}]  ", filter_lbl), Style::default().fg(Color::DarkGray)),
        Span::styled("r ", Style::default().fg(Color::Cyan)),
        Span::styled("Refresh  ", Style::default().fg(Color::DarkGray)),
        Span::styled("q ", Style::default().fg(Color::Cyan)),
        Span::styled("Quit  ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("  {}", refresh_lbl), Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)),
    ]);

    f.render_widget(
        Paragraph::new(line)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))),
        area,
    );
}

// ─── Body: sidebar + detail panel ────────────────────────────────────────────

fn render_body(f: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(22), Constraint::Percentage(78)])
        .split(area);

    // Sidebar
    {
        let indices = app.filtered_indices();
        let items: Vec<ListItem> = indices.iter()
            .filter_map(|&idx| app.interfaces.get(idx))
            .map(|i| {
                let is_up = i.operstate == "up";
                let (dot, col) = if is_up { ("●", Color::Green) } else { ("○", Color::Red) };
                let st = if is_up { "UP  " } else { "DOWN" };

                // Show live rate if UP
                let rate_hint = if is_up {
                    app.history.get(&i.name)
                        .map(|h| {
                            let r = h.current_rx() + h.current_tx();
                            if r > 0 { format!(" {}", fmt_rate(r)) } else { String::new() }
                        })
                        .unwrap_or_default()
                } else { String::new() };

                ListItem::new(Line::from(vec![
                    Span::styled(format!(" {} ", dot), Style::default().fg(col)),
                    Span::styled(format!("{:<9}", i.name), Style::default().fg(Color::White)),
                    Span::styled(st, Style::default().fg(col).add_modifier(Modifier::DIM)),
                    Span::styled(rate_hint, Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)),
                ]))
            })
            .collect();

        let filter_lbl = if app.show_all { "All" } else { "UP" };
        let title = format!(" Interfaces [{}] ", filter_lbl);
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title)
                .border_style(Style::default().fg(Color::Blue)))
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol("▶ ");
        f.render_stateful_widget(list, cols[0], &mut app.list_state);
    }

    // Detail panel
    render_detail_panel(f, app, cols[1]);
}

// ─── Detail Panel (tabs) ─────────────────────────────────────────────────────

fn render_detail_panel(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let tab_titles = vec![
        Line::from(Span::raw("  [1] Details  ")),
        Line::from(Span::raw("  [2] Traffic  ")),
        Line::from(Span::raw("  [3] Packets  ")),
    ];
    let iface_name = app.selected_iface().map(|i| i.name.as_str()).unwrap_or("—");
    let panel_title = format!(" {} ", iface_name);

    let tabs = Tabs::new(tab_titles)
        .block(Block::default().borders(Borders::ALL).title(panel_title)
            .border_style(Style::default().fg(Color::Blue)))
        .select(app.tab.index())
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, rows[0]);

    match app.tab {
        Tab::Details => render_tab_details(f, app, rows[1]),
        Tab::Traffic => render_tab_traffic(f, app, rows[1]),
        Tab::Packets => render_tab_packets(f, app, rows[1]),
    }
}

// ─── Tab: Details ────────────────────────────────────────────────────────────

fn render_tab_details(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let Some(i) = app.selected_iface() else {
        f.render_widget(
            Paragraph::new("\n  ← Select an interface from the list")
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::ALL).title(" Details ")
                    .border_style(Style::default().fg(Color::Blue))),
            area,
        );
        return;
    };

    let is_up = i.operstate == "up";
    let (status_fg, status_bg) = if is_up { (Color::Black, Color::Green) } else { (Color::Black, Color::Red) };
    let status_text = if is_up { " ● UP " } else { " ○ DOWN " };

    let speed_display = fmt_speed(&i.speed);
    let ipv4_str = if i.ipv4.is_empty() { "—".to_string() } else { i.ipv4.join("  ") };

    let mut lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Interface    : ", Style::default().fg(Color::Yellow)),
            Span::styled(i.name.clone(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Status       : ", Style::default().fg(Color::Yellow)),
            Span::styled(status_text, Style::default().fg(status_fg).bg(status_bg).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  MAC Address  : ", Style::default().fg(Color::Yellow)),
            Span::styled(i.mac.clone(), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  MTU          : ", Style::default().fg(Color::Yellow)),
            Span::styled(format!("{} bytes", i.mtu), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Link Speed   : ", Style::default().fg(Color::Yellow)),
            Span::styled(speed_display, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  IPv4         : ", Style::default().fg(Color::Yellow)),
            Span::styled(ipv4_str, Style::default().fg(Color::Cyan)),
        ]),
    ];

    // IPv6 — one per line
    lines.push(Line::from(""));
    if i.ipv6.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  IPv6         : ", Style::default().fg(Color::Yellow)),
            Span::styled("—", Style::default().fg(Color::DarkGray)),
        ]));
    } else {
        for (idx, addr) in i.ipv6.iter().enumerate() {
            let label = if idx == 0 { "  IPv6         : ".to_string() } else { format!("  {:15}: ", "") };
            lines.push(Line::from(vec![
                Span::styled(label, Style::default().fg(Color::Yellow)),
                Span::styled(addr.clone(), Style::default().fg(Color::Blue)),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ──────────── Traffic Summary ─────────────────────────────────────────", Style::default().fg(Color::DarkGray)),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  RX Total     : ", Style::default().fg(Color::Yellow)),
        Span::styled(fmt_bytes(i.rx_bytes), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::styled(format!("   ({} pkts)", i.rx_packets), Style::default().fg(Color::DarkGray)),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  TX Total     : ", Style::default().fg(Color::Yellow)),
        Span::styled(fmt_bytes(i.tx_bytes), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        Span::styled(format!("   ({} pkts)", i.tx_packets), Style::default().fg(Color::DarkGray)),
    ]));

    let err_total = i.rx_errors + i.tx_errors + i.rx_dropped + i.tx_dropped;
    lines.push(Line::from(""));
    let (health_icon, health_text, health_col) = if err_total == 0 {
        ("✓", " No errors or drops detected", Color::Green)
    } else {
        ("⚠", " Errors/drops detected — see Packets tab", Color::Yellow)
    };
    lines.push(Line::from(vec![
        Span::styled("  Health       : ", Style::default().fg(Color::Yellow)),
        Span::styled(health_icon, Style::default().fg(health_col).add_modifier(Modifier::BOLD)),
        Span::styled(health_text, Style::default().fg(health_col)),
    ]));

    f.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(" Details ")
                .border_style(Style::default().fg(Color::Blue)))
            .wrap(Wrap { trim: false }),
        area,
    );
}

// ─── Tab: Traffic ─────────────────────────────────────────────────────────────

fn render_tab_traffic(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let Some(iface) = app.selected_iface() else {
        f.render_widget(
            Paragraph::new("\n  ← Select an interface")
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::ALL).title(" Traffic ")
                    .border_style(Style::default().fg(Color::Blue))),
            area,
        );
        return;
    };

    let (cur_rx, cur_tx, rx_hist, tx_hist, peak_rx, peak_tx) =
        match app.history.get(&iface.name) {
            Some(h) => (h.current_rx(), h.current_tx(), h.rx_history.clone(), h.tx_history.clone(), h.peak_rx(), h.peak_tx()),
            None    => (0, 0, vec![0u64; HISTORY_SIZE], vec![0u64; HISTORY_SIZE], 0, 0),
        };

    let speed_mbps: u64 = iface.speed.trim().parse().unwrap_or(1000);
    let speed_bytes = (speed_mbps * 1_000_000 / 8).max(1);
    let rx_pct = ((cur_rx * 100) / speed_bytes).min(100) as u16;
    let tx_pct = ((cur_tx * 100) / speed_bytes).min(100) as u16;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // live rates box
            Constraint::Length(3), // rx gauge
            Constraint::Length(3), // tx gauge
            Constraint::Min(4),    // sparklines
        ])
        .split(area);

    // ── Live rates ──
    let rate_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  ▼ RX  ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:>14}", fmt_rate(cur_rx)), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(format!("   cumulative: {}", fmt_bytes(iface.rx_bytes)), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ▲ TX  ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:>14}", fmt_rate(cur_tx)), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::styled(format!("   cumulative: {}", fmt_bytes(iface.tx_bytes)), Style::default().fg(Color::DarkGray)),
        ]),
    ];
    f.render_widget(
        Paragraph::new(rate_lines)
            .block(Block::default().borders(Borders::ALL)
                .title(format!(" Live Rate — 1s interval  (link: {}) ", fmt_speed(&iface.speed)))
                .border_style(Style::default().fg(Color::Blue))),
        chunks[0],
    );

    // ── Gauges ──
    f.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(" ▼ RX Utilization ")
                .border_style(Style::default().fg(Color::Blue)))
            .gauge_style(Style::default().fg(gauge_color(rx_pct)).bg(Color::Black))
            .percent(rx_pct)
            .label(format!("{}% of {}", rx_pct, fmt_speed(&iface.speed))),
        chunks[1],
    );
    f.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(" ▲ TX Utilization ")
                .border_style(Style::default().fg(Color::Blue)))
            .gauge_style(Style::default().fg(gauge_color(tx_pct)).bg(Color::Black))
            .percent(tx_pct)
            .label(format!("{}% of {}", tx_pct, fmt_speed(&iface.speed))),
        chunks[2],
    );

    // ── Sparklines ──
    let spark_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[3]);

    f.render_widget(
        Sparkline::default()
            .block(Block::default().borders(Borders::ALL)
                .title(format!(" ▼ RX History ({}s)   peak: {} ", HISTORY_SIZE, fmt_rate(peak_rx)))
                .border_style(Style::default().fg(Color::Blue)))
            .data(&rx_hist)
            .max(peak_rx.max(1))
            .style(Style::default().fg(Color::Green)),
        spark_chunks[0],
    );
    f.render_widget(
        Sparkline::default()
            .block(Block::default().borders(Borders::ALL)
                .title(format!(" ▲ TX History ({}s)   peak: {} ", HISTORY_SIZE, fmt_rate(peak_tx)))
                .border_style(Style::default().fg(Color::Blue)))
            .data(&tx_hist)
            .max(peak_tx.max(1))
            .style(Style::default().fg(Color::Magenta)),
        spark_chunks[1],
    );
}

// ─── Tab: Packets ─────────────────────────────────────────────────────────────

fn render_tab_packets(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let Some(i) = app.selected_iface() else {
        f.render_widget(
            Paragraph::new("\n  ← Select an interface")
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::ALL).title(" Packets ")
                    .border_style(Style::default().fg(Color::Blue))),
            area,
        );
        return;
    };

    let sep = "  ─────────────────────────────────────────────────────────";
    let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD | Modifier::UNDERLINED);

    let err_col   = |v: u64| if v > 0 { Color::Red    } else { Color::Green };
    let drop_col  = |v: u64| if v > 0 { Color::Yellow } else { Color::Green };

    let row = |label: &str, rx: String, tx: String, rx_col: Color, tx_col: Color| -> Line {
        Line::from(vec![
            Span::styled(format!("  {:<22}", label), Style::default().fg(Color::White)),
            Span::styled(format!("{:>20}", rx), Style::default().fg(rx_col)),
            Span::styled(format!("{:>20}", tx), Style::default().fg(tx_col)),
        ])
    };

    let err_total = i.rx_errors + i.tx_errors;
    let drop_total = i.rx_dropped + i.tx_dropped;

    let mut lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("  {:<22}", "Metric"), header_style),
            Span::styled(format!("{:>20}", "RX"), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
            Span::styled(format!("{:>20}", "TX"), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
        ]),
        Line::from(sep),
        Line::from(""),
        row("Packets", i.rx_packets.to_string(), i.tx_packets.to_string(), Color::Green, Color::Magenta),
        Line::from(""),
        row("Bytes", fmt_bytes(i.rx_bytes), fmt_bytes(i.tx_bytes), Color::Green, Color::Magenta),
        Line::from(""),
        row("Errors", i.rx_errors.to_string(), i.tx_errors.to_string(), err_col(i.rx_errors), err_col(i.tx_errors)),
        Line::from(""),
        row("Dropped", i.rx_dropped.to_string(), i.tx_dropped.to_string(), drop_col(i.rx_dropped), drop_col(i.tx_dropped)),
        Line::from(""),
        Line::from(sep),
        Line::from(""),
    ];

    // Health summary
    if err_total == 0 && drop_total == 0 {
        lines.push(Line::from(vec![
            Span::styled("  ✓ ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("Interface is healthy — zero errors and zero drops", Style::default().fg(Color::Green)),
        ]));
    } else {
        if err_total > 0 {
            lines.push(Line::from(vec![
                Span::styled("  ⚠ ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{} total error(s) detected (RX: {}  TX: {})", err_total, i.rx_errors, i.tx_errors),
                    Style::default().fg(Color::Red)),
            ]));
        }
        if drop_total > 0 {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  ⚠ ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{} total drop(s) detected (RX: {}  TX: {})", drop_total, i.rx_dropped, i.tx_dropped),
                    Style::default().fg(Color::Yellow)),
            ]));
        }
    }

    f.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(" Packet Statistics ")
                .border_style(Style::default().fg(Color::Blue)))
            .wrap(Wrap { trim: false }),
        area,
    );
}