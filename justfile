default:
    @just --list

[no-cd]
backtrace-test:
    RUSTFLAGS="-Zmacro-backtrace" cargo test

