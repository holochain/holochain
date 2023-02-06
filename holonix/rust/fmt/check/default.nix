{ pkgs }:
let
  name = "hn-rust-fmt-check";

  script = pkgs.writeShellScriptBin name ''
    echo "checking rust formatting"
    cargo fmt -- --check
  '';
in
{ buildInputs = [ script ]; }
