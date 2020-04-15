#!/usr/bin/env bash

# assumes that the following environment variables are set and legit
# - $DOCKER_USER
# - $DOCKER_PASS
# - $AWS_ACCESS_KEY_ID
# - $AWS_SECRET_ACCESS_KEY

set -euo pipefail
dir=$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )

echo "$DOCKER_PASS" | docker login --username $DOCKER_USER --password-stdin

# while read region; do
#  $( aws ecr get-login --no-include-email --region $region )
# done < $dir/aws-regions.txt
