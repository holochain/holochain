#!/usr/bin/env nix-shell
#! nix-shell -I nixpkgs=https://github.com/NixOS/nixpkgs/archive/nixos-23.11.tar.gz
#! nix-shell -i bash --pure
#! nix-shell -p bash taplo

# Usage to format: ./scripts/format-toml.sh
# Usage to check: ./scripts/format-toml.sh --check

set -eux

EXTRA_ARG=${1:-}

taplo format "$EXTRA_ARG" ./*.toml
taplo format "$EXTRA_ARG" ./crates/**/*.toml
