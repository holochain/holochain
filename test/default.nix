{ pkgs }:
let
  name = "hcp-test";

  script = pkgs.writeShellScriptBin name
  ''
  set -euxo pipefail
  CARGO_TARGET_DIR=test_utils/wasm/target/foo cargo build --release --manifest-path test_utils/wasm/foo/Cargo.toml --target wasm32-unknown-unknown -Z unstable-options

  RUST_BACKTRACE=1 \
  hn-rust-fmt-check \
  && hn-rust-clippy \
  && cargo test
  '';
in
{
 buildInputs = [ script ];
}
