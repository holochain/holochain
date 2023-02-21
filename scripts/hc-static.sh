#!/bin/sh
cargo fmt --check && \
  cargo clippy -- \
  -A clippy::nursery \
  -D clippy::style \
  -A clippy::cargo \
  -A clippy::pedantic \
  -A clippy::restriction \
  -D clippy::complexity \
  -D clippy::perf \
  -D clippy::correctness

