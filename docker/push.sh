#!/usr/bin/env bash

set -euxo pipefail
dir=$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )

owner=holochain
repo=holochain-2020
box=${1:-latest}
branch=${2:-$( git rev-parse --abbrev-ref HEAD )}
tag="$owner/$repo:$box.$branch"

docker push "holochain/holochain-2020:$box.$branch"
