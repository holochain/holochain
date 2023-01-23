{ pkgs }:
let
  name = "hn-flush";

  script = pkgs.writeShellScriptBin name ''
    hn-node-flush
    hn-rust-flush
  '';
in { buildInputs = [ script ]; }
