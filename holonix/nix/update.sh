#! /usr/bin/env nix-shell
#! nix-shell ../default.nix
#! nix-shell --pure
#! nix-shell --arg include "{ node = false; holochainBinaries = false; launcher = false; scaffolding = false; rust = false; }"
#! nix-shell --keep GITHUB_TOKEN
#! nix-shell -i bash

set -ex
niv update ${@}

if git diff --exit-code --quiet ../nix/sources.*; then
    echo no changes, exiting..
    exit 0
fi

nix/regen_versions.sh

cat << EOF | git commit VERSIONS.md nix/sources.* -F -
update nix sources

see VERSIONS.md for the exact changes
EOF
