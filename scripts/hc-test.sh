#!/bin/sh
cmd="cargo nextest --config-file .config/nextest.toml run $(cat .config/test-args.nix | xargs) $(cat .config/nextest-args.nix | xargs)"
$cmd
