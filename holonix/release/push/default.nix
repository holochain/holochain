{ pkgs, config }:
let
  name = "hn-release-push";

  script = pkgs.writeShellScriptBin name ''
    set -euo pipefail
    echo
    echo "kicking off release ${config.release.tag}"
    echo
    if [ "$(git rev-parse --abbrev-ref HEAD)" == "${config.release.branch}" ]
     then
      git add . && git commit -am 'Release ${config.release.tag}'
      git push
      git pull ${config.release.upstream} master
      git push ${config.release.upstream} HEAD:master
      git pull ${config.release.upstream} develop
      git push ${config.release.upstream} HEAD:develop
      echo
      echo "tagging ${config.release.tag}"
      git tag -a ${config.release.tag} -m "Version ${config.release.tag}"
      git push ${config.release.upstream} ${config.release.tag}
      echo
      echo "release tags pushed"

      # finish on a fresh develop
      git checkout develop
      git pull

     else
      echo "current branch is not ${config.release.branch}!";
      exit 1;
    fi
  '';
in { buildInputs = [ script ]; }
