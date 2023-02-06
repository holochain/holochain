{ pkgs, config }:
let
  name = "hn-release-hook-preflight-manual";

  script = pkgs.writeShellScriptBin name ''
    echo
    read -r -p "Are you sure you want to cut a new release based on the current config? [y/N] " response
    case "$response" in
     [yY][eE][sS]|[yY])
     ;;
     *)
     exit 1
     ;;
    esac
  '';
in
{ buildInputs = [ script ]; }
