use crate::data::{Interface, TrafficHistory};
use crate::system_reader::get_interfaces;
use ratatui::widgets::ListState;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Details,
    Traffic,
    Packets,
}

impl Tab {
    pub fn next(self) -> Self {
        match self {
            Self::Details => Self::Traffic,
            Self::Traffic => Self::Packets,
            Self::Packets => Self::Details,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Self::Details => Self::Packets,
            Self::Traffic => Self::Details,
            Self::Packets => Self::Traffic,
        }
    }
    pub fn index(self) -> usize {
        match self {
            Self::Details => 0,
            Self::Traffic => 1,
            Self::Packets => 2,
        }
    }
}

pub struct App {
    pub interfaces: Vec<Interface>,
    pub list_state: ListState,
    pub history: HashMap<String, TrafficHistory>,
    pub tab: Tab,
    pub show_all: bool, // true = all, false = UP only
    pub refresh_count: u64,
}

impl App {
    pub fn new() -> Self {
        let ifaces = get_interfaces();
        let mut list_state = ListState::default();
        if !ifaces.is_empty() {
            list_state.select(Some(0));
        }
        let mut history = HashMap::new();
        for i in &ifaces {
            history.insert(i.name.clone(), TrafficHistory::new(i.rx_bytes, i.tx_bytes));
        }
        Self {
            interfaces: ifaces,
            list_state,
            history,
            tab: Tab::Details,
            show_all: true,
            refresh_count: 0,
        }
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        self.interfaces
            .iter()
            .enumerate()
            .filter(|(_, i)| self.show_all || i.operstate == "up")
            .map(|(idx, _)| idx)
            .collect()
    }

    pub fn selected_iface(&self) -> Option<&Interface> {
        let f = self.filtered_indices();
        self.list_state
            .selected()
            .and_then(|s| f.get(s))
            .and_then(|&idx| self.interfaces.get(idx))
    }

    pub fn navigate(&mut self, delta: i64) {
        let n = self.filtered_indices().len();
        if n == 0 {
            return;
        }
        let cur = self.list_state.selected().unwrap_or(0) as i64;
        let next = ((cur + delta).rem_euclid(n as i64)) as usize;
        self.list_state.select(Some(next));
    }

    pub fn refresh(&mut self) {
        self.interfaces = get_interfaces();
        for i in &self.interfaces {
            let h = self.history
                .entry(i.name.clone())
                .or_insert_with(|| TrafficHistory::new(i.rx_bytes, i.tx_bytes));
            h.update(i.rx_bytes, i.tx_bytes);
        }
        let n = self.filtered_indices().len();
        if let Some(s) = self.list_state.selected() {
            if n == 0 {
                self.list_state.select(None);
            } else if s >= n {
                self.list_state.select(Some(n - 1));
            }
        }
        self.refresh_count += 1;
    }

    pub fn stats_summary(&self) -> (usize, usize) {
        let up = self.interfaces.iter().filter(|i| i.operstate == "up").count();
        (up, self.interfaces.len())
    }
}