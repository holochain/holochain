#!/usr/bin/env bash

set -e

export NIX_INSTALLER_URL=${NIX_INSTALLER_URL:-https://releases.nixos.org/nix/nix-2.28.3/install}

run_cmd() {
    echo "$@"
    "$@"
}

append_nix_experimental_features() {
    local nix_conf
    local features
    nix_conf=${NIX_CONF_PATH:-"$HOME/.config/nix/nix.conf"}
    features="$@"
    # check if nix.conf exists
    if [ -f "$nix_conf" ]; then
        # case when nix.conf exists
        # check if experimental-features line exists
        if grep -q "^experimental-features" "$nix_conf"; then
            # case when experimental-features line exists
            # get current features
            features_list=$(grep "^experimental-features" "$nix_conf" | cut -d '=' -f 2 | sed 's/^ *//')
            # append new features if not already present
            for feature in $features; do
                if ! echo " $features_list " | grep -q " $feature "; then
                    features_list="$features_list $feature"
                fi
            done
            # update nix.conf with new features
            # we need to use -i.bak for compatibility with both GNU sed and BSD sed (macOS)
            run_cmd sed -i.bak "s|^experimental-features = .*|experimental-features = $features_list|" "$nix_conf"
            # so we can remove the backup file
            run_cmd rm "$nix_conf.bak"
        else
            # case when experimental-features line does not exist
            # add new line
            run_cmd bash -c "echo 'experimental-features = $features' >> \"$nix_conf\""
        fi
    else
        # case when nix.conf does not exist
        # create nix.conf with features
        run_cmd mkdir -p ~/.config/nix
        run_cmd bash -c "echo 'experimental-features = $features' >> \"$nix_conf\""
    fi
}

main() {
    if ! command -v nix &>/dev/null; then
        echo "Nix package manager not found"
        echo "Installing Nix"
        echo

        run_cmd bash <(curl -L "${NIX_INSTALLER_URL}") --daemon

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
    run_cmd sudo --preserve-env=NIX_CONFIG,PATH "$(which nix)" run nixpkgs/nixos-25.11#cachix -- use holochain-ci -m root-nixconf
    echo

    echo "Restarting Nix daemon"
    if command -v systemctl &> /dev/null; then
      run_cmd sudo systemctl restart nix-daemon
    elif command -v launchctl &> /dev/null; then
      run_cmd sudo launchctl kickstart -k system/org.nixos.nix-daemon
    else
      # Fallback which should work on most systems
      run_cmd sudo pkill nix-daemon || :
    fi
    echo

    echo "Enabling additional Nix commands and Nix flakes"
    append_nix_experimental_features "nix-command" "flakes"
    echo

    echo "Please close this shell and open a new one to start using Nix".
}

# Execute main if script is run directly
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main
fi
