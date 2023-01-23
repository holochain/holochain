{ pkgs, config }:
let
  bump-version = pkgs.writeShellScriptBin "hn-release-hook-version-rust" ''
    set -euxo pipefail
    echo "bumping Cargo versions to ${config.release.version.current} in Cargo.toml"
    find . \
     -name "Cargo.toml" \
     -not -path "**/.git/**" \
     -not -path "**/.cargo/**" | xargs -I {} \
     sed -i 's/^\s*version\s*=\s*"[0-9]\+.[0-9]\+.[0-9]\+\(-alpha[0-9]\+\)\?"\s*$/version = "${config.release.version.current}"/g' {}
  '';

  bump-deps = pkgs.writeShellScriptBin "hn-release-hook-version-rust-deps" ''
    set -euxo pipefail
    for dep in ''${1}
    do
    echo "bumping $dep dependency versions to ${config.release.version.current} in all Cargo.toml"
    find . \
     -name "Cargo.toml" \
     -not -path "**/target/**" \
     -not -path "**/.git/**" \
     -not -path "**/.cargo/**" | xargs -I {} \
     sed -i 's/^'"''${dep}"' = { version = "=[0-9]\+.[0-9]\+.[0-9]\+\(-alpha[0-9]\+\)\?"/'"''${dep}"' = { version = "=${config.release.version.current}"/g' {}
    done
  '';
in { buildInputs = [ bump-version bump-deps ]; }
