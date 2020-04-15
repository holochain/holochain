#!/usr/bin/env bash

set -euxo pipefail

owner=holochain
repo=holochain-2020
box=${1:-sim2h_server}
branch=${2:-$( git rev-parse --abbrev-ref HEAD )}
tag="$owner/$repo:$box.$branch"

docker build ./docker --build-arg DOCKER_BRANCH=$branch --build-arg GITHUB_ACCESS_TOKEN=$GITHUB_ACCESS_TOKEN -f ./docker/Dockerfile.$box -t $tag --no-cache
