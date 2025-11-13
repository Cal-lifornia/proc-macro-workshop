default:
    @just --list

[no-cd]
backtrace-test:
    RUST_BACKTRACE=1 RUSTFLAGS="-Zproc-macro-backtrace" cargo test

backtrace-run:
    RUST_BACKTRACE=1 RUSTFLAGS="-Z macro-backtrace" cargo run

