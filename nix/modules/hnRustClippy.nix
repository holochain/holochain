{ self, lib, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }: {
    packages.hnRustClippy = pkgs.writeShellScriptBin "hn-rust-clippy" ''
      cargo clippy --target-dir "$CARGO_TARGET_DIR/clippy" -- \
      -A clippy::nursery -D clippy::style -A clippy::cargo \
      -A clippy::pedantic -A clippy::restriction \
      -D clippy::complexity -D clippy::perf -D clippy::correctness
    '';
  };
}
