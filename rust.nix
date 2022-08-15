{ pkgs ? import <nixpkgs> { } }:
with pkgs;

pkgs.mkShell {
  buildInputs = [
    (rust-bin.stable.latest.default.override {
      extensions = [ "rust-src" ];
      targets = [ "wasm32-unknown-unknown" ];
    })
    pkgs.darwin.apple_sdk.frameworks.AppKit
  ];
}
