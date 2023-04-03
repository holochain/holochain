#!/usr/bin/env bash

set -xeu

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

podman build --build-arg USERNAME="$USER" --tag ubuntu-nix "${SCRIPT_DIR}"
xhost +si:localuser:"$USER"
podman run -it --rm \
  --userns keep-id \
  --security-opt label=type:container_runtime_t \
  -v "$XAUTHORITY":"$XAUTHORITY":ro \
  -v /tmp/.X11-unix:/tmp/.X11-unix:ro \
  --env XAUTHORITY \
  --env DISPLAY \
  --env NIX_REMOTE="daemon" \
  --env NIX_CONFIG="experimental-features = nix-command flakes" \
  -v /nix/store:/nix/store:ro \
  -v /nix/var/nix/db:/nix/var/nix/db:ro \
  -v /nix/var/nix/daemon-socket:/nix/var/nix/daemon-socket:ro \
  -v "$(readlink "$(which nix)")":/usr/bin/nix:ro \
  -v "${SCRIPT_DIR}"/../..:/home/"$USER"/project \
  --workdir /home/"$USER"/project \
  localhost/ubuntu-nix \
  bash

