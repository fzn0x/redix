# Redix

![redix](assets/imgs/redix.png)

A simplified, distributed key-value store built in Rust, implementing the **Raft Consensus Protocol**.

## 🚀 Features

-   **Distributed Consensus**: Uses Raft for leader election and log replication.
-   **Redis Compatible**: Supports basic `SET` and `GET` commands via any standard Redis client.
-   **In-Memory Storage**: Fast, thread-safe storage using `DashMap`.
-   **Configurable**: Easily specify node IDs, ports, and peer addresses via CLI.

## 🛠️ Installation

### Prerequisites

-   [Rust](https://www.rust-lang.org/tools/install) (2024 edition)
-   `cargo`

### Build

```bash
git clone https://github.com/fzn0x/redix.git
cd redix
cargo build --release
```

## 📖 Usage Guide

### Single Node (Standalone)

To run as a single node, you must specify `--peers-count 1` so the node can elect itself as leader:

```bash
cargo run -- --id 0 --peers-count 1 --raft-addr 127.0.0.1:8080 --redis-addr 127.0.0.1:6380
```

### Multi-Node Cluster (3 Nodes)

Start each node in a separate terminal:

**Node 0:**
```bash
cargo run -- --id 0 --raft-addr 127.0.0.1:8000 --redis-addr 127.0.0.1:6379 --peer 1=127.0.0.1:8001 --peer 2=127.0.0.1:8002
```

**Node 1:**
```bash
cargo run -- --id 1 --raft-addr 127.0.0.1:8001 --redis-addr 127.0.0.1:6380 --peer 0=127.0.0.1:8000 --peer 2=127.0.0.1:8002
```

**Node 2:**
```bash
cargo run -- --id 2 --raft-addr 127.0.0.1:8002 --redis-addr 127.0.0.1:6381 --peer 0=127.0.0.1:8000 --peer 1=127.0.0.1:8001
```

### Interacting with the Server

You can use the built-in interactive CLI or standard tools:

#### Built-in Interactive CLI (Recommended)
```bash
cargo run -- --id 0 --peers-count 1 --redis-addr 127.0.0.1:6380
cargo run --bin cli -- --addr 127.0.0.1:6380
# Inside the CLI:
redix> SET foo bar
redix> GET foo
redix> KEYS
redix> FLUSHALL
redix> DEL foo
```

#### Using Rust Example
```bash
cargo run --example basic_usage
```

#### Using Node.js Example
```bash
cd examples/nodejs
npm install
npm start
```

## 🧪 Testing

Run the full test suite (including integration cluster tests):

```bash
cargo test
```

## 📜 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
