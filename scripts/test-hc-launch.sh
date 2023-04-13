#!/usr/bin/env bash

# TODO: build the hc-launch-test.webhapp file from source
echo pass | nix develop .#holonix --command hc-launch --piped -n1 ./hc-launch-test.webhapp network mdns
