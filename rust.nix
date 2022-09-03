with import <nixpkgs> { };

mkShell {
  buildInputs = [
    (rust-bin.stable.latest.default.override {
      extensions = [ "rust-src" ];
      targets = [ "wasm32-unknown-unknown" ];
    })
    pkgs.darwin.apple_sdk.frameworks.AppKit
    pkgs.cargo-readme
    pkgs.cargo-nextest
    pkgs.clippy
    # pkgs.nodejs-16_x
  ];
}
