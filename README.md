# TFM Rust plugin

An async WASI Preview 3 analyzer plugin for Rust source files. It extracts
English source messages from `t!("...")` macros and returns locations through
the `tfm:plugin@0.1.0` contract.

```sh
cargo test
cargo build --release --target wasm32-wasip2
wasm-tools validate target/wasm32-wasip2/release/tfm_plugin_rust.wasm
```

The plugin receives source text from the host and has no filesystem, network,
environment, Git, LSP, or LLM capability.

The WIT contract is vendored from [`tfm`](https://github.com/tfm-git/tfm) at
commit `74b45b6`. A release process must update it only together with a reviewed
ABI version change.
