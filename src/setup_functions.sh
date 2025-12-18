#!/usr/bin/env bash

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
