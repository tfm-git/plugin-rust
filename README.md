# TFM Rust plugin

An analyzer plugin for Rust source files. It compiles to `wasm32-wasip2` and
exports an async WASI Preview 3 WIT/component ABI through `wit-bindgen 0.60.0`.
It extracts
English source messages from `t!("...")` macros and returns locations plus an
AST-local anchor through the `tfm:plugin@0.2.0` contract.

```sh
cargo test
cargo build --release --target wasm32-wasip2
wasm-tools validate target/wasm32-wasip2/release/tfm_plugin_rust.wasm
```

The plugin receives source text from the host and has no filesystem, network,
environment, Git, LSP, or LLM capability.

The WIT contract is vendored from [`tfm`](https://github.com/tfm-git/tfm) ABI
v0.3.0. A release process must update it only together with a reviewed ABI
version change.
