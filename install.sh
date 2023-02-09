#!/usr/bin/env bash

cat <<EOF
Welcome, in the future this might be used to help set up the Holochain development environment.
This script doesn't run any commands for you, it only displays them.

EOF

if ! type -f nix >/dev/null; then
cat <<EOF
# Install the nix utility.
sh <(curl -L https://nixos.org/nix/install) --daemon
EOF
fi

cat <<EOF

# Prepare the environment, setup the binary cache and make sure it's loaded
export NIX_CONFIG="extra-experimental-features = nix-command flakes"
sudo --preserve-env=PATH $(which nix) run nixpkgs/nixos-22.11#cachix -- use holochain-ci -m root-nixconf && sudo pkill nix-daemon
EOF

cat <<EOF

# This will scaffold the example project into the 'forum' directory
nix run github:holochain/holochain#hc-scaffold -- example forum
EOF
