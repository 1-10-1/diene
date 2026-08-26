# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Diene is a work-in-progress Rust game engine with a Vulkan renderer backend, structured as a Cargo workspace of small, single-purpose crates.

## Commands

The pinned toolchain is nightly (`rust-toolchain.toml`); rustfmt relies on unstable options, so always use the pinned toolchain rather than stable rustfmt/clippy.

- Full local gate (mirrors CI): `just ci` — runs `fmt-check -> check -> clippy -> test -> doc -> deny`.
- Quick iteration: `cargo check --workspace --all-targets --all-features` (alias `cargo c`).
- Lint: `cargo clippy --workspace --all-targets --all-features -- -D warnings` (alias `cargo cl`).
- Test: `cargo test --workspace --all-targets --all-features` (alias `cargo t`); a single test: `cargo test -p <crate> <test_name> --all-features`.
- `cargo nextest run --workspace --all-features` is also configured (`just nextest`), with a stricter CI profile via `cargo nextest run --profile ci`.
- Note that testing is ignored in this project, so don't bother.
- Docs: `cargo docx` (alias for `cargo doc --workspace --all-features --no-deps`) builds docs; the CI/`just doc` gate additionally sets `RUSTDOCFLAGS=-D warnings` (broken intra-doc links, missing docs on public items) — the alias alone won't reproduce that failure locally, set the env var or use `just doc`.
- Supply-chain check: `cargo deny check` (licenses, advisories, banned wildcards).
- Run the sandbox app: `cargo run -p sandbox` (alias `cargo r`).
- `lefthook.yml` runs `fmt` + `check` on pre-commit and `just ci` on pre-push — expect these gates locally, not just in CI.

On `x86_64-unknown-linux-gnu`, `.cargo/config.toml` pins the `clang` + `mold` linker; both must be installed or builds fail. It also sets `RUST_LOG` (trace for `diene_*`/`sandbox` targets), and `common::logging::init()` writes to `logs/diene.log` relative to the current working directory.

## Architecture

- **`engine-renderer-api`** is the contract crate: the `Renderer`/`RendererFactory`/`RenderWindow` traits and backend-neutral data types (`RenderScene`, `RenderObject`, `RenderCamera`, `MeshData`, `MaterialData`, `TextureData`, ...). It has no dependency on any concrete backend or on `engine-core`.
- **`engine-renderer-vulkan`** implements those traits with `ash`/`vk-mem`. It's the only crate where `unsafe` is permitted, and every unsafe block/fn must carry a safety doc comment (`clippy::undocumented_unsafe_blocks` / `missing_safety_doc` are denied there).
- **`engine-shader`** is a standalone Slang-to-SPIR-V compiler, also backend-neutral; `engine-renderer-vulkan` consumes it, but nothing about it is Vulkan-specific.
- **`engine-core`** owns the winit event loop and window lifecycle (`ApplicationHost`). It depends only on `engine-renderer-api` and `common`, never on a concrete backend — it stays generic over `Renderer`/`RendererFactory` and type-erases them internally (see the `Erased*`/`*Adapter` types in `app.rs`) so the host binary doesn't need to know the backend's concrete error type.
- **`engine-runtime`** is where a concrete backend actually gets wired in: it depends on `engine-core`, `engine-renderer-api`, and `engine-renderer-vulkan`, and owns backend-selection policy (`RendererBackend::Auto` / `Vulkan`). A new backend gets plugged in here, not in `engine-core`.
- **`diene`** is the public facade crate (`#![forbid(unsafe_code)]`) that re-exports curated pieces of the above under `app`, `renderer`, `shader`, and a `prelude` module — this is what downstream consumers of the engine are expected to depend on.
- **`common`** holds shared `tracing`-based logging setup and a `Stopwatch` timer, and is depended on by nearly every other crate.
- **`sandbox`** is the private (`publish = false`) dev binary used to exercise the engine end-to-end; it builds a demo scene directly against `engine-runtime`/`engine-renderer-api`.

The dependency direction is one-way: contract/utility crates (`engine-renderer-api`, `engine-shader`, `common`) know nothing about concrete backends; `engine-core` knows only the contract; only `engine-runtime` and backend crates know about concrete implementations like Vulkan. When extending the renderer, prefer adding to `engine-renderer-api` (and implementing it in the backend crate) over reaching into `engine-core` or `engine-runtime` for backend-specific concerns.

## Conventions

- `engine-renderer-vulkan` is the only crate that permits `unsafe`; every other engine/facade crate (`diene`, `engine-core`, `engine-renderer-api`, `engine-runtime`, `engine-shader`, `sandbox`) opts in to `#![forbid(unsafe_code)]` — keep it that way rather than loosening it elsewhere.
- Prefer `parking_lot::{Mutex, RwLock}` over `std::sync` equivalents, and avoid `std::mem::forget`/`std::process::exit` (enforced via `clippy.toml` disallowed-types/methods).
- `unwrap_used`, `todo!`, `unimplemented!`, and `panic_in_result_fn` are denied by workspace lints (`Cargo.toml`); library code should build `thiserror` error enums and thread them through `error_stack::Result`/`Report` rather than panicking. The `sandbox` binary is the exception, converting to `anyhow` at the top and pretty-printing the error chain in `main.rs`.
- Public items need rustdoc (`missing_docs = "warn"` workspace-wide); `sandbox` opts out with `#![allow(missing_docs)]` since it's a binary, not a library.
- Shader sources live under `shaders/` (Slang, e.g. `shaders/main.slang`); compiled `*.spv` output is gitignored, so don't assume generated shader binaries are tracked.
