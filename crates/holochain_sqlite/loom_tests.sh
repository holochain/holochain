#!/usr/bin/env bash
RUSTFLAGS="--cfg loom" cargo test --test loom --no-default-features --features test_utils --release -- --nocapture
