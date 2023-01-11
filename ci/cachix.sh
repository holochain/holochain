#! /usr/bin/env nix-shell
#! nix-shell -i bash -p bash -p cachix -I nixpkgs="channel:nixos-21.05"

## nix-shell -i bash -p "((import ./config.nix).holochain-nixpkgs.importFn {}).pkgs.cachix"

set -euo pipefail

export PATHS_PREBUILD_FILE="${HOME}/.store-path-pre-build"
export NIX_PATH=nixpkgs=$(nix eval --raw '((import ./config.nix).holochain-nixpkgs.pathFn {})')

case ${1} in
  setup)
    if [[ -n ${CACHIX_AUTH_TOKEN:-} ]]; then
        echo Using CACHIX_AUTH_TOKEN
        cachix --verbose authtoken ${CACHIX_AUTH_TOKEN}
    fi
    cachix --verbose use -m user-nixconf ${CACHIX_NAME:?}
    nix path-info --all > "${PATHS_PREBUILD_FILE}"
    ;;

  push)
    comm -13 <(sort "${PATHS_PREBUILD_FILE}" | grep -v '\.drv$') <(nix path-info --all | grep -v '\.drv$' | sort) | cachix --verbose push ${CACHIX_NAME:?}
    ;;
esac
