#!/usr/bin/env bash
set -euo pipefail

# Export the conductor API TypeScript bindings.
# Usage: scripts/export-ts-bindings.sh <output-dir>
#
# Runs a single aggregate export test in holochain_conductor_api whose
# `export_ts_bindings` walks every participating crate. The whole tree must
# be written by one process: ts-rs only merges declarations that share an
# output file within a single process, so separate per-crate export runs
# would truncate each other's files. For the same reason the test must run
# under plain `cargo test` — nextest spawns one process per test and would
# reintroduce the truncation.
#
# `unstable-countersigning` is enabled on top of `ts_rs` so the countersigning
# app API reaches the bindings. The session state types are exported either
# way, and without the feature the `AppRequest`/`AppResponse` variants that
# return them are compiled out, leaving the client with types no generated
# request can produce.
EXPORT_DIR="${1:?usage: export-ts-bindings.sh <output-dir>}"
mkdir -p "$EXPORT_DIR"
EXPORT_DIR="$(cd "$EXPORT_DIR" && pwd)"

# Export into a staging directory so the output directory is never left in a
# half-written state if the export fails.
STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT

TS_RS_EXPORT_DIR="$STAGING" \
  cargo test -p holochain_conductor_api --features ts_rs,unstable-countersigning export_bindings_aggregate

# Replace the output directory wholesale so declarations from a previous run
# whose types have since been renamed or removed don't linger.
rm -rf "$EXPORT_DIR"
mkdir -p "$(dirname "$EXPORT_DIR")"
cp -r "$STAGING" "$EXPORT_DIR"

echo "TypeScript bindings written to $EXPORT_DIR"
