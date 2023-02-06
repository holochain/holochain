{ pkgs }:
let
  name = "hn-rust-manifest-set-ver";

  script = pkgs.writeShellScriptBin name ''
    # node dist can mess with the process
    hn-flush
    find . -name "Cargo.toml" | xargs -I {} cargo upgrade "$1" --all --manifest-path {}
  '';
in
{ buildInputs = [ script ]; }
