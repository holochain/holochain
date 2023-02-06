{ pkgs }:
let
  name = "hn-rust-manifest-list-unpinned";

  script = pkgs.writeShellScriptBin name ''
    find . -type f \( -name "Cargo.toml" -or -name "Cargo.template.toml" \) -not -path "./.cargo/*" | xargs cat | grep -Ev '=[0-9]+\.[0-9]+\.[0-9]+' | grep -E '[0-9]+' | grep -Ev '(version|edition|codegen-units|{ git = ".*", rev = "\w+" })' | cat
  '';
in
{ buildInputs = [ script ]; }
