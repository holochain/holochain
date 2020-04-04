{ pkgs }:
let

  wasms = [ "foo" "imports" ];

  build-wasm = name:
  ''
  set -euxo pipefail
  CARGO_TARGET_DIR=test_utils/wasm/target/${name} \
  cargo build \
    --release \
    --manifest-path test_utils/wasm/${name}/Cargo.toml \
    --target wasm32-unknown-unknown
  '';
  build-wasms = pkgs.writeShellScriptBin "hc-test-wasms-build"
  (pkgs.lib.concatMapStrings build-wasm wasms);

  t0 = pkgs.writeShellScriptBin "hc-test"
  ''
  set -euxo pipefail
  RUST_BACKTRACE=1 cargo test -- --nocapture
  '';

  test-wasm = pkgs.writeShellScriptBin "hc-test-wasm"
  ''
  set -euxo pipefail
  hc-test-wasms-build
  RUST_BACKTRACE=1 cargo test --manifest-path crates/holochain/Cargo.toml --features wasmtest -- --nocapture
  '';

  t1 = pkgs.writeShellScriptBin "hc-merge-test"
  ''
  RUST_BACKTRACE=1 \
  hn-rust-fmt-check \
  && hn-rust-clippy \
  && hc-test \
  && hc-test-wasm
  '';
in
{
 buildInputs = [ t0 t1 build-wasms test-wasm ];
}
