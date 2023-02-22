#!/bin/sh

name="./scripts/$(basename $0)"
if test ! -f "$name"; then
  echo "ERROR: run from root holochain directory: '$name'"
  exit 127
fi

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

