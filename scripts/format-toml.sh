#!/usr/bin/env nix-shell
#! nix-shell -i bash --pure
#! nix-shell -p bash taplo

EXTRA_ARGS=$1

taplo format ./*.toml "$EXTRA_ARGS"
taplo format ./crates/**/*.toml "$EXTRA_ARGS"
