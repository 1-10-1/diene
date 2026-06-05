set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

alias c := check
alias t := test

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

check:
    cargo check --workspace --all-targets --all-features

clippy:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
    cargo test --workspace --all-targets --all-features

nextest:
    cargo nextest run --workspace --all-features

doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

deny:
    cargo deny check

machete:
    cargo machete

ci: fmt-check check clippy test doc deny
