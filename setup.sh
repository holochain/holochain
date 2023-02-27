#!/usr/bin/env bash

set -e

if ! command -v nix &>/dev/null; then
    echo "Nix package manager not found"
    echo "Installing Nix"
    echo

    if [[ $(uname -r) == *"WSL2" ]]; then
        echo "bash <(curl -L https://nixos.org/nix/install) --no-daemon"
        bash <(curl -L https://nixos.org/nix/install) --no-daemon
    else
        echo "bash <(curl -L https://nixos.org/nix/install) --daemon"
        bash <(curl -L https://nixos.org/nix/install) --daemon
    fi

    echo
    echo "Sourcing the nix config files"
    source /etc/profile
    for file in \
        /etc/profile \
        /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh \
        /nix/var/nix/profiles/default/etc/profile.d/nix.sh
    do
        source $file > /dev/null 2>&1 || :
    done
    echo
fi

echo "Setting up binary cache for all users (requires root access)"
echo "sudo --preserve-env=PATH $(which nix) run nixpkgs/nixos-22.11#cachix --extra-experimental-features \"nix-command flakes\" -- use holochain-ci -m root-nixconf"
sudo --preserve-env=PATH $(which nix) run nixpkgs/nixos-22.11#cachix --extra-experimental-features "nix-command flakes" -- use holochain-ci -m root-nixconf
echo

echo "Restarting Nix daemon"
echo "sudo pkill nix-daemon"
sudo pkill nix-daemon || :
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
