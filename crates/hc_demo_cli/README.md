# hc_demo_cli

`hc demo-cli` provides a self-contained demo for experiencing and testing holochain without depending
on downstream projects such as launcher.

The demo is composed of a simple file sharing zome and a CLI to package it into a holochain app and run it.

## Usage

Run `hc demo-cli help` to get started.

## Rebuilding the WASM

Currently, the demo uses a compiled zome that is checked into version control.

To rebuild the WASMs:

```
cd crates/hc_demo_cli
RUSTFLAGS="--cfg build_wasm" cargo build
```
