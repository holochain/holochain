#!/bin/sh

name="./scripts/$(basename $0)"
if test ! -f "$name"; then
  echo "ERROR: run from root holochain directory: '$name'"
  exit 127
fi

cmd="cargo nextest --config-file .config/nextest.toml run $(cat .config/test-args.nix | xargs) $(cat .config/nextest-args.nix | xargs) --features tx5"
$cmd
