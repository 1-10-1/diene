# AGENTS.md

## Project rules
- This is a Rust game engine workspace.
- Prefer explicit, well-documented public APIs.
- Run `cargo check --workspace` after code changes.
- `engine_core` must not depend directly on renderer backend details.
- Public structs/functions need concise Rustdoc.

## Commands
- Use the pinned nightly toolchain from `rust-toolchain.toml`; rustfmt uses unstable options.
- Full local gate: `just ci` runs `fmt-check -> check -> clippy -> test -> doc -> deny`.
- Faster pre-commit equivalent: `cargo fmt --all -- --check` then `cargo check --workspace --all-targets --all-features`.
- Run Clippy as configured: `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- Run tests: `cargo test --workspace --all-targets --all-features`; focused test: `cargo test -p <crate> <filter> --all-features`.
- `cargo nextest run --workspace --all-features` is configured, with CI profile available as `cargo nextest run --profile ci`.
- Run the app with `cargo run -p sandbox` or repo alias `cargo r`.

## Workspace Shape
- Workspace members are `common`, `engine-renderer-vulkan`, `engine-core`, and `sandbox`; the default member is `sandbox`.
- `sandbox/src/main.rs` is the executable entrypoint and builds an `engine_core::app::Application`.
- `engine-core` owns app lifecycle/orchestration and currently depends on the Vulkan renderer.
- `common` owns shared logging/timing utilities.
- `engine-renderer-vulkan` is the renderer backend crate; unsafe is not forbidden there, but unsafe docs/blocks are strictly linted.

## Repo Gotchas
- On `x86_64-unknown-linux-gnu`, `.cargo/config.toml` uses `clang` with `mold`; missing either can break builds.
- `.cargo/config.toml` sets `RUST_LOG=trace`, and `common::logging::init()` writes `logs/diene.log` in the current working directory.
- Workspace lints are strict: `unwrap`, `todo`, `unimplemented`, undocumented unsafe, and many Clippy warnings will fail under `just ci`.
- Prefer `parking_lot::{Mutex,RwLock}` over `std::sync::{Mutex,RwLock}` per `clippy.toml`.
- `*.spv` and `*.log` are ignored; do not assume generated shader binaries or logs are tracked.
