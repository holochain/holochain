#!/bin/sh

name="./scripts/$(basename $0)"
if test ! -f "$name"; then
  echo "ERROR: run from root holochain directory: '$name'"
  exit 127
fi

cargo fmt --check && cargo clippy -- $(xargs <.config/clippy-args.nix)
