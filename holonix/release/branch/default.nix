{ pkgs, config }:
let

  name = "hn-release-branch";

  upstream = "origin";

  script = pkgs.writeShellScriptBin name ''
    set -euo pipefail
    echo
    echo 'preparing release branch'
    echo
    git fetch
    if git tag | grep -q "${config.release.branch}"
    then echo "There is a tag with the same name as the release branch ${config.release.branch}! aborting..."
    exit 1;
    fi;
    echo
    echo 'checkout or create release branch'
    if git branch | grep -q "${config.release.branch}"
     then git checkout ${config.release.branch}
      git pull;
     else git checkout ${config.release.commit}
      git checkout -b ${config.release.branch}
      git pull ${config.release.upstream} master
      git pull ${config.release.upstream} develop
      git push -u ${config.release.upstream} ${config.release.branch};
    fi;
    echo
  '';
in { buildInputs = [ script ]; }
