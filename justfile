set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

alias c := check

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

check:
    cargo check --workspace --all-features

clippy:
    cargo clippy --workspace --all-features -- -D warnings

doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

deny:
    cargo deny check

machete:
    cargo machete

ci: fmt-check check clippy doc deny
