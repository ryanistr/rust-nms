# rust-nms-tui

## What is NMS
A Network Management System (NMS) application monitors, maintains, and optimizes network infrastructure. It provides visibility into device status, traffic flow, and system metrics to detect anomalies and manage network configurations.

## Why Rust
Rust provides deterministic performance and memory safety without a garbage collector. It prevents data races in concurrent network polling operations. The compiled binary is standalone and requires no runtime dependencies, making it optimal for cross-platform deployment, including resource-constrained or embedded environments.

## Features
* **Terminal UI**: Ratatui-based layout with asynchronous event polling.
* **Sysfs Polling**: 1-second interval hardware metrics extraction from `/sys/class/net`.
* **State Filtering**: Toggle view between all interfaces or active (UP) interfaces only.
* **Details View**: Hardware MAC extraction, MTU limits, link speed negotiation, and IPv4/IPv6 address resolution.
* **Traffic View**: Real-time RX/TX rate calculation, link utilization gauges, and graphical sparkline history.
* **Packets View**: Granular tracking of cumulative packets, total bytes, transmission errors, and packet drops.
* **System Telemetry**: Aggregated interface health indicators and system uptime tracking.
* **Keyboard Navigation**: Vim-bindings (`j`, `k`), arrow keys, numeric tab switching (`1`, `2`, `3`), and direct filters (`f`).

## Installation

### 1. Manual Compilation
Ensure `cargo` and `rustc` are installed.

```bash
git clone https://github.com/ryanistr/rust-nms
cd rust-nms
cargo build --release
```
The binary is generated at target/release/rust-nms-tui.

### 2. Automatic Compilation
The repository includes a script to automate building, stripping, and UPX compression.

```bash
git clone https://github.com/ryanistr/rust-nms
cd rust-nms
chmod +x compile.sh
./compile.sh
```

The compiled binary `rust-nms-tui` is placed in the project root directory. 
To compile for Android, execute `./compile.sh -a` (requires Android NDK mapped).

### 3. Download Release

Download the pre-compiled binary directly from the repository's [Release page](https://github.com/ryanistr/rust-nms/releases/). Extract the archive if compressed.

## Execution

### Run Locally

Execute the binary directly from the current directory:

```bash
./rust-nms-tui
```

### Run Globally

Move the binary to a directory in your system's `PATH` to execute it from any terminal session.

```bash
mkdir -p ~/.local/bin
cp rust-nms-tui ~/.local/bin/
```

Ensure `~/.local/bin` is exported in your `.bashrc` or `.zshrc`:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Execute from anywhere:
```bash
rust-nms-tui
```