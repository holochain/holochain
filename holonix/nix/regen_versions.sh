#! /usr/bin/env nix-shell
#! nix-shell --pure --keep NIX_PATH
#! nix-shell -p cacert nixUnstable
#! nix-shell -p niv -p yq -i bash

set -e

export NIX_CONFIG="extra-experimental-features = nix-command";

# read the ids from holochain-nixpkg's CI configuration
# all of these are verified to build and cached
yq_input="$(nix eval --impure --raw --expr '(builtins.toString (import nix/sources.nix).holochain-nixpkgs)')/.github/workflows/build.yml"
ids="$(yq -r <${yq_input} '.jobs."holochain-binaries".strategy.matrix.nixAttribute | join(" ")')"

cat << EOF > VERSIONS.md
# Holonix Version Information

## Common binaries
The following binaries are the same version regardless of the _holochainVersionId_ argument.

$(nix-shell --pure --run 'hn-introspect common')

## Available _holochainVersionId_ options
Each of the following headings represent one pre-built _holochainVersionId_ and their corresponding holochain version information.

$(
    for id in ${ids}; do
        printf '### '
        nix-shell --pure --argstr holochainVersionId ${id} --run 'hn-introspect hc'
        printf '\n'
    done
)
EOF
