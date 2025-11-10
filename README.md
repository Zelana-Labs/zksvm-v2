# ⚡ zkSVM v2

A modular, multi-crate Rust workspace implementing the zkSVM framework — including the core rollup logic, RPC server, benchmarking tools, and service orchestration.

---

## 🚀 Getting Started

### 1️⃣ Prerequisites

Make sure you have:
- [Rust](https://rustup.rs/) (latest stable toolchain)
- [LLVM / Clang](https://llvm.org/docs/GettingStarted.html)

> **Note:** `librocksdb-sys` requires LLVM headers and a working C++ toolchain.

---

### 2️⃣ Build the Project

From the workspace root:
```bash
cargo build
```

To build in release mode:

```bash
cargo build --release
```

---

## 🧱 Workspace Structure

| Crate         | Type    | Description                   |
| ------------- | ------- | ----------------------------- |
| `rpc_server`  | Service | Handles API / RPC requests    |
| `bon`         | Service | Core rollup node orchestrator |
| `rollup-core` | Library | Core zkSVM + rollup logic     |
| `bench-tool`  | CLI     | Benchmarking & testing tools  |
| `debug-db`    | CLI     | Database debugging tools      |

---

## ⚙️ Running the Services

### 🧩 RPC Server

```
cargo run -p rpc_server
```

---

### ⚙️ BON — Core Service

```powershell
# Initialize configuration (one-time)
cargo run -p bon --bin init_config

# Run the main BON node
cargo run -p bon --bin bon
```

---

### 🧪 Bench Tool

```
cargo run -p bench-tool 
```

Used for local testing and performance benchmarks.

---

### 🧩 Rollup Core

`rollup-core` is a library crate, used internally by other services.
You can run its tests:

```
cargo test -p rollup-core
```

---

### 🧩 Debug DB

Look at the data in the RocksDB

```
cargo run -p debug-db
```

---

## 🧰 Development Notes

* Logs:

  ```
  $env:RUST_LOG = "debug"
  ```
* Clean build:

  ```
  cargo clean
  ```
* Test everything:

  ```
  cargo test
  ```

---

## 📄 License

This project is licensed under the MIT License.
See [LICENSE](./LICENSE) for more information.