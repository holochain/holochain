{ pkgs }:
let
  name = "hn-rust-fmt";

  script = pkgs.writeShellScriptBin name ''
    cargo fmt
  '';
in { buildInputs = [ script ]; }
