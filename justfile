default:
    @just --list

[no-cd]
backtrace-test:
    RUST_BACKTRACE=1 RUSTFLAGS="-Z macro-backtrace" cargo test
