#!/usr/bin/env bash

if ! type -f nix >/dev/null; then
cat <<EOF
# Install the nix utility first:
sh <(curl -L https://nixos.org/nix/install) --daemon
EOF
fi

echo "Setting up the binary cache..."
sudo --preserve-env=PATH $(which nix) run nixpkgs/nixos-22.11#cachix --extra-experimental-features nix-command --extra-experimental-features flakes -- use holochain-ci -m root-nixconf && sudo pkill nix-daemon

echo "Scaffolding the example Holochain app..."
nix run github:holochain/holochain#hc-scaffold -- example forum
