{ pkgs, config }:
let
  name = "hn-release-cut";

  script = pkgs.writeShellScriptBin name ''
    set -euo pipefail

    echo "** START PREFLIGHT HOOK **"
    ${config.release.hook.preflight}
    echo "** END PREFLIGHT HOOK **"

    hn-release-branch
    hn-release-changelog

    echo "** START VERSION HOOK **"
    ${config.release.hook.version}
    echo "** END VERSION HOOK **"

    hn-release-push
    hn-release-github

    echo "** START PUBLISH HOOK **"
    ${config.release.hook.publish}
    echo "** END PUBLISH HOOK **"
  '';
in
{ buildInputs = [ script ]; }
