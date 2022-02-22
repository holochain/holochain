#! /usr/bin/env nix-shell
#! nix-shell ../shell.nix
#! nix-shell --fallback
#! nix-shell --pure
#! nix-shell --argstr flavor "coreDev"
#! nix-shell -i bash
set +e
git diff --exit-code
hc-release-test
