#!/usr/bin/env sh

set -e

if ! command -v nix &>/dev/null; then
    echo "Nix package manager not found"
    echo "Installing Nix"
    echo
    echo "sh <'(curl -L https://nixos.org/nix/install)' --daemon"
    sh <'(curl -L https://nixos.org/nix/install)' --daemon

    echo
    echo "Starting Nix daemon"
    source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh > /dev/null 2>&1
    source /nix/var/nix/profiles/default/etc/profile.d/nix.sh > /dev/null 2>&1
    echo
fi

echo "Setting up binary cache for all users (requires root access)"
echo "sudo --preserve-env=PATH $(which nix) run nixpkgs/nixos-22.11#cachix --extra-experimental-features \"nix-command flakes\" -- use holochain-ci -m root-nixconf"
sudo --preserve-env=PATH $(which nix) run nixpkgs/nixos-22.11#cachix --extra-experimental-features "nix-command flakes" -- use holochain-ci -m root-nixconf
echo

echo "Restarting Nix daemon"
echo "sudo pkill nix-daemon"
sudo pkill nix-daemon
echo

echo "Creating Nix user config in ~/.config/nix/nix.conf"
echo "mkdir -p ~/.config/nix"
mkdir -p ~/.config/nix
echo

echo "Enabling additional Nix commands and Nix flakes"
echo "echo \"experimental-features = nix-command flakes\" >>~/.config/nix/nix.conf"
echo "experimental-features = nix-command flakes" >>~/.config/nix/nix.conf
echo

echo "Please close this shell and open a new one to start using Nix".
