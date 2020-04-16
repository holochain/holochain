#!/usr/bin/env bash

# assumes that the following environment variables are set and legit
# - $DOCKER_USER
# - $DOCKER_PASS

set -euo pipefail
dir=$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )

echo "$DOCKER_PASS" | docker login --username $DOCKER_USER --password-stdin
