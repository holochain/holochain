{ pkgs }:
let

  t0 = pkgs.writeShellScriptBin "hc-test"
  ''
  set -euxo pipefail
  RUST_BACKTRACE=1 \
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
