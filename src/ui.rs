use crate::app::{App, Tab};
use crate::constants::HISTORY_SIZE;
use crate::formatters::{fmt_bytes, fmt_rate, fmt_speed, gauge_color};
use crate::system_reader::{get_hostname, get_uptime_str};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline, Tabs, Wrap},
};

pub fn ui(f: &mut ratatui::Frame, app: &mut App) {
    let area = f.size();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    render_header(f, app, rows[0]);
    render_body(f, app, rows[1]);
    render_footer(f, app, rows[2]);
}

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
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            ),
        area,
    );
}

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
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            ),
        area,
    );
}

fn render_body(f: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(22), Constraint::Percentage(78)])
        .split(area);

    {
        let indices = app.filtered_indices();
        let items: Vec<ListItem> = indices
            .iter()
            .filter_map(|&idx| app.interfaces.get(idx))
            .map(|i| {
                let is_up = i.operstate == "up";
                let (dot, col) = if is_up { ("●", Color::Green) } else { ("○", Color::Red) };
                let st = if is_up { "UP  " } else { "DOWN" };

                let rate_hint = if is_up {
                    app.history
                        .get(&i.name)
                        .map(|h| {
                            let r = h.current_rx() + h.current_tx();
                            if r > 0 {
                                format!(" {}", fmt_rate(r))
                            } else {
                                String::new()
                            }
                        })
                        .unwrap_or_default()
                } else {
                    String::new()
                };

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
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol("▶ ");
        f.render_stateful_widget(list, cols[0], &mut app.list_state);
    }

    render_detail_panel(f, app, cols[1]);
}

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
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(panel_title)
                .border_style(Style::default().fg(Color::Blue)),
        )
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

fn render_tab_details(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let Some(i) = app.selected_iface() else {
        f.render_widget(
            Paragraph::new("\n  ← Select an interface from the list")
                .style(Style::default().fg(Color::DarkGray))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Details ")
                        .border_style(Style::default().fg(Color::Blue)),
                ),
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
    lines.push(Line::from(vec![Span::styled(
        "  ──────────── Traffic Summary ─────────────────────────────────────────",
        Style::default().fg(Color::DarkGray),
    )]));
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
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Details ")
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_tab_traffic(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let Some(iface) = app.selected_iface() else {
        f.render_widget(
            Paragraph::new("\n  ← Select an interface")
                .style(Style::default().fg(Color::DarkGray))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Traffic ")
                        .border_style(Style::default().fg(Color::Blue)),
                ),
            area,
        );
        return;
    };

    let (cur_rx, cur_tx, rx_hist, tx_hist, peak_rx, peak_tx) = match app.history.get(&iface.name) {
        Some(h) => (
            h.current_rx(),
            h.current_tx(),
            h.rx_history.clone(),
            h.tx_history.clone(),
            h.peak_rx(),
            h.peak_tx(),
        ),
        None => (0, 0, vec![0u64; HISTORY_SIZE], vec![0u64; HISTORY_SIZE], 0, 0),
    };

    let speed_mbps: u64 = iface.speed.trim().parse().unwrap_or(1000);
    let speed_bytes = (speed_mbps * 1_000_000 / 8).max(1);
    let rx_pct = ((cur_rx * 100) / speed_bytes).min(100) as u16;
    let tx_pct = ((cur_tx * 100) / speed_bytes).min(100) as u16;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(4),
        ])
        .split(area);

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
        Paragraph::new(rate_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Live Rate — 1s interval  (link: {}) ", fmt_speed(&iface.speed)))
                .border_style(Style::default().fg(Color::Blue)),
        ),
        chunks[0],
    );

    f.render_widget(
        Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" ▼ RX Utilization ")
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .gauge_style(Style::default().fg(gauge_color(rx_pct)).bg(Color::Black))
            .percent(rx_pct)
            .label(format!("{}% of {}", rx_pct, fmt_speed(&iface.speed))),
        chunks[1],
    );
    f.render_widget(
        Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" ▲ TX Utilization ")
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .gauge_style(Style::default().fg(gauge_color(tx_pct)).bg(Color::Black))
            .percent(tx_pct)
            .label(format!("{}% of {}", tx_pct, fmt_speed(&iface.speed))),
        chunks[2],
    );

    let spark_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[3]);

    f.render_widget(
        Sparkline::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" ▼ RX History ({}s)   peak: {} ", HISTORY_SIZE, fmt_rate(peak_rx)))
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .data(&rx_hist)
            .max(peak_rx.max(1))
            .style(Style::default().fg(Color::Green)),
        spark_chunks[0],
    );
    f.render_widget(
        Sparkline::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" ▲ TX History ({}s)   peak: {} ", HISTORY_SIZE, fmt_rate(peak_tx)))
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .data(&tx_hist)
            .max(peak_tx.max(1))
            .style(Style::default().fg(Color::Magenta)),
        spark_chunks[1],
    );
}

fn render_tab_packets(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let Some(i) = app.selected_iface() else {
        f.render_widget(
            Paragraph::new("\n  ← Select an interface")
                .style(Style::default().fg(Color::DarkGray))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Packets ")
                        .border_style(Style::default().fg(Color::Blue)),
                ),
            area,
        );
        return;
    };

    let sep = "  ─────────────────────────────────────────────────────────";
    let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD | Modifier::UNDERLINED);

    let err_col = |v: u64| if v > 0 { Color::Red } else { Color::Green };
    let drop_col = |v: u64| if v > 0 { Color::Yellow } else { Color::Green };

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

    if err_total == 0 && drop_total == 0 {
        lines.push(Line::from(vec![
            Span::styled("  ✓ ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("Interface is healthy — zero errors and zero drops", Style::default().fg(Color::Green)),
        ]));
    } else {
        if err_total > 0 {
            lines.push(Line::from(vec![
                Span::styled("  ⚠ ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{} total error(s) detected (RX: {}  TX: {})", err_total, i.rx_errors, i.tx_errors),
                    Style::default().fg(Color::Red),
                ),
            ]));
        }
        if drop_total > 0 {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  ⚠ ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{} total drop(s) detected (RX: {}  TX: {})", drop_total, i.rx_dropped, i.tx_dropped),
                    Style::default().fg(Color::Yellow),
                ),
            ]));
        }
    }

    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Packet Statistics ")
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}