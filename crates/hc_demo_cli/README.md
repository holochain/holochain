# hc_demo_cli

## Rebuilding the WASM

Currently we're checking the generated WASM for the demo into version control.

To Rebuild:

```
cd crates/hc_demo_cli
RUSTFLAGS="--cfg build_wasm" cargo build
```
