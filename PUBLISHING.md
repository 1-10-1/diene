# Publishing

Diene is published as a small crate family. Publish from the workspace root after the local gate passes.

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo package --workspace --allow-dirty
```

Publish crates in dependency order:

```sh
cargo publish -p diene-common
cargo publish -p diene-engine-renderer-api
cargo publish -p diene-engine-core
cargo publish -p diene-engine-renderer-vulkan
cargo publish -p diene-engine-runtime
cargo publish -p diene
```

The `sandbox` package is private and intentionally has `publish = false`.
