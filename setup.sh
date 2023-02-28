#!/usr/bin/env bash

set -e

run_cmd() {
    echo "$@"
    "$@"
}

if ! command -v nix &>/dev/null; then
    echo "Nix package manager not found"
    echo "Installing Nix"
    echo

    if [[ $(uname -r) == *"WSL2" ]]; then
        run_cmd bash <(curl -L https://nixos.org/nix/install) --no-daemon
    else
        run_cmd bash <(curl -L https://nixos.org/nix/install) --daemon
    fi

    echo
    echo "Sourcing the nix config files"
    set +e
    for file in \
        ~/.profile \
        /etc/profile \
        /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh \
        /nix/var/nix/profiles/default/etc/profile.d/nix.sh
    do
        source $file > /dev/null 2>&1
        if command -v nix; then break; fi
    done
    set -e
    echo
fi

echo "Setting up binary cache for all users (requires root access)"
run_cmd sudo --preserve-env=NIX_CONFIG,PATH $(which nix) run nixpkgs/nixos-22.11#cachix --extra-experimental-features "nix-command flakes" -- use holochain-ci -m root-nixconf
echo

echo "Restarting Nix daemon"
run_cmd sudo pkill nix-daemon || :
echo

echo "Creating Nix user config in ~/.config/nix/nix.conf"
run_cmd mkdir -p ~/.config/nix
echo

echo "Enabling additional Nix commands and Nix flakes"
run_cmd bash -c 'echo "experimental-features = nix-command flakes" >>~/.config/nix/nix.conf'
echo

echo "Please close this shell and open a new one to start using Nix".
