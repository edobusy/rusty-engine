# Rusty Engine

![Rust](https://img.shields.io/badge/Rust-2024_edition-orange?logo=rust&logoColor=white)
![License](https://img.shields.io/badge/license-MIT-blue)
![Status](https://img.shields.io/badge/status-work_in_progress-yellow)

A matching engine for environmental commodity spot exchanges, built in Rust.

Implements price-time priority (CLOB) and uniform price auction matching, with per-product configuration, O(1) cancellation, and a command-event architecture.

Work in progress.

## Build & Test

```bash
cargo build
cargo test
cargo clippy
```

## License

[MIT](LICENSE)
