#!/usr/bin/env bash

cd "$(dirname "$0")/.."

source ./setup.sh

# make temp file for nix conf
nix_conf_temp=$(mktemp /tmp/nix.conf.XXXXXX)
export NIX_CONF_PATH="$nix_conf_temp"

# test case 1: nix.conf does not exist
append_nix_experimental_features "nix-command" "flakes"

expected="experimental-features = nix-command flakes"
result=$(cat "$nix_conf_temp")
if [ "$result" != "$expected" ]; then
    echo "Test case 1 failed: expected '$expected', got '$result'"
    exit 1
fi

# test case 2: nix.conf exists without experimental-features
echo "parameter = value" > "$nix_conf_temp"
append_nix_experimental_features "nix-command" "flakes"

expected="parameter = value
experimental-features = nix-command flakes"
result=$(cat "$nix_conf_temp")
if [ "$result" != "$expected" ]; then
    echo "Test case 2 failed: expected '$expected', got '$result'"
    exit 1
fi

# test case 3: nix.conf exists with experimental-features
echo "parameter = value" > "$nix_conf_temp"
echo "experimental-features = old-feature" >> "$nix_conf_temp"
append_nix_experimental_features "nix-command" "flakes"

expected="parameter = value
experimental-features = old-feature nix-command flakes"
result=$(cat "$nix_conf_temp")
if [ "$result" != "$expected" ]; then
    echo "Test case 3 failed: expected '$expected', got '$result'"
    exit 1
fi

# cleanup
rm "$nix_conf_temp"

echo "All tests passed."
