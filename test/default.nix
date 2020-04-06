{ pkgs }:
let

  t0 = pkgs.writeShellScriptBin "hc-test"
  ''
  set -euxo pipefail
  RUST_BACKTRACE=1 \
  CARGO_TARGET_DIR=test_utils/wasm/target/foo \
  cargo build --release --manifest-path test_utils/wasm/foo/Cargo.toml --target wasm32-unknown-unknown && \
  cargo test -- --nocapture
  '';

  t1 = pkgs.writeShellScriptBin "hc-merge-test"
  ''
  RUST_BACKTRACE=1 \
  hn-rust-fmt-check \
  && hn-rust-clippy \
  && hc-test
  '';
in
{
 buildInputs = [ t0 t1 ];
}
