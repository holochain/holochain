#!/usr/bin/env bash

if ! command -v nix &>/dev/null; then
    echo "Nix package manager not found"
    echo "Install Nix first, or open a new shell if it is already installed"
    echo
    echo "sh <(curl -L https://nixos.org/nix/install) --daemon"
    exit 1
fi

echo
echo "Setting up binary cache for all users (requires root access)"
sudo --preserve-env=PATH $(which nix) run nixpkgs/nixos-22.11#cachix --extra-experimental-features nix-command --extra-experimental-features flakes -- use holochain-ci -m root-nixconf && sudo pkill nix-daemon
echo

echo "Creating Nix user config in ~/.config/nix/nix.conf"
mkdir -p ~/.config/nix
echo

echo "Enabling additional Nix commands and Nix flakes"
echo "experimental-features = nix-command flakes" >~/.config/nix/nix.conf
echo

echo "Scaffolding the example Holochain app"
nix run github:holochain/holochain#hc-scaffold -- example forum
