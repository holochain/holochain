#!/usr/bin/env nix-shell
#! nix-shell -I nixpkgs=https://github.com/NixOS/nixpkgs/archive/nixos-23.11.tar.gz
#! nix-shell -i bash --pure
#! nix-shell -p bash taplo

set -eux

EXTRA_ARGS=$1

taplo format ./*.toml "$EXTRA_ARGS"
taplo format ./crates/**/*.toml "$EXTRA_ARGS"
