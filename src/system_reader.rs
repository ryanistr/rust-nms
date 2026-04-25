use crate::data::Interface;
use std::{fs, process::Command};

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
            if let Some(a) = t.split_whitespace().nth(1) {
                v6.push(a.to_string());
            }
        } else if t.starts_with("inet ") {
            if let Some(a) = t.split_whitespace().nth(1) {
                v4.push(a.to_string());
            }
        }
    }
    (v4, v6)
}

pub fn get_interfaces() -> Vec<Interface> {
    let Ok(entries) = fs::read_dir("/sys/class/net") else {
        return vec![];
    };
    let mut list: Vec<Interface> = entries
        .flatten()
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let p = entry.path();
            let s = p.join("statistics");
            let (ipv4, ipv6) = get_ip_addresses(&name);
            Interface {
                operstate: read_file(&p, "operstate"),
                mac: read_file(&p, "address"),
                mtu: read_file(&p, "mtu").parse().unwrap_or(0),
                speed: read_file(&p, "speed"),
                rx_bytes: read_stat_u64(&s, "rx_bytes"),
                tx_bytes: read_stat_u64(&s, "tx_bytes"),
                rx_packets: read_stat_u64(&s, "rx_packets"),
                tx_packets: read_stat_u64(&s, "tx_packets"),
                rx_errors: read_stat_u64(&s, "rx_errors"),
                tx_errors: read_stat_u64(&s, "tx_errors"),
                rx_dropped: read_stat_u64(&s, "rx_dropped"),
                tx_dropped: read_stat_u64(&s, "tx_dropped"),
                name,
                ipv4,
                ipv6,
            }
        })
        .collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

pub fn get_hostname() -> String {
    fs::read_to_string("/etc/hostname")
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub fn get_uptime_str() -> String {
    let raw = fs::read_to_string("/proc/uptime").unwrap_or_default();
    let secs = raw
        .split_whitespace()
        .next()
        .and_then(|x| x.parse::<f64>().ok())
        .unwrap_or(0.0) as u64;
    let (d, h, m, s) = (
        secs / 86400,
        (secs % 86400) / 3600,
        (secs % 3600) / 60,
        secs % 60,
    );
    if d > 0 {
        format!("{}d {}h {}m", d, h, m)
    } else if h > 0 {
        format!("{}h {}m {:02}s", h, m, s)
    } else {
        format!("{}m {:02}s", m, s)
    }
}