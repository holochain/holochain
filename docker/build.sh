#!/usr/bin/env bash

set -euxo pipefail

owner=holochain
repo=holochain
box=${1:-latest}
branch=${2:-$( git rev-parse --abbrev-ref HEAD )}
tag="$owner/$repo:$box.$branch"

docker build ./docker \
    --build-arg DOCKER_BRANCH=$branch \
    --build-arg CACHIX_NAME="${CACHIX_NAME}" \
    --build-arg NIX_CONFIG="${NIX_CONFIG}" \
    -f ./docker/Dockerfile.$box -t $tag --no-cache
