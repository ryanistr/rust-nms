use crate::constants::HISTORY_SIZE;

#[derive(Clone, Default)]
pub struct Interface {
    pub name: String,
    pub operstate: String,
    pub mac: String,
    pub mtu: u64,
    pub speed: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub rx_dropped: u64,
    pub tx_dropped: u64,
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
}

pub struct TrafficHistory {
    pub rx_history: Vec<u64>,
    pub tx_history: Vec<u64>,
    pub prev_rx: u64,
    pub prev_tx: u64,
}

impl TrafficHistory {
    pub fn new(rx: u64, tx: u64) -> Self {
        Self {
            rx_history: vec![0u64; HISTORY_SIZE],
            tx_history: vec![0u64; HISTORY_SIZE],
            prev_rx: rx,
            prev_tx: tx,
        }
    }

    pub fn update(&mut self, rx: u64, tx: u64) {
        let rx_rate = rx.saturating_sub(self.prev_rx);
        let tx_rate = tx.saturating_sub(self.prev_tx);
        self.rx_history.push(rx_rate);
        self.tx_history.push(tx_rate);
        if self.rx_history.len() > HISTORY_SIZE {
            self.rx_history.remove(0);
        }
        if self.tx_history.len() > HISTORY_SIZE {
            self.tx_history.remove(0);
        }
        self.prev_rx = rx;
        self.prev_tx = tx;
    }

    pub fn current_rx(&self) -> u64 {
        self.rx_history.last().copied().unwrap_or(0)
    }
    pub fn current_tx(&self) -> u64 {
        self.tx_history.last().copied().unwrap_or(0)
    }
    pub fn peak_rx(&self) -> u64 {
        self.rx_history.iter().copied().max().unwrap_or(0)
    }
    pub fn peak_tx(&self) -> u64 {
        self.tx_history.iter().copied().max().unwrap_or(0)
    }
}