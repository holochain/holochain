#!/usr/bin/env bash

set -euox pipefail

VERSION_STR=$1

cd "$(git rev-parse --show-toplevel)"

if [ ! -f "./versions/$VERSION_STR/flake.nix" ]; then
    echo "File not found: ./versions/$VERSION_STR/flake.nix"
    exit 1
fi

SEP_COUNT=$(echo "$VERSION_STR" | tr -d -c '_' | awk '{ print length; }')

SEARCH_PATTERN="no-tag"
if [ "$SEP_COUNT" == "1" ]; then
    # Matched format 0_X
    SEARCH_PATTERN=$(echo "$VERSION_STR" | awk -F _ 'BEGIN { OFS="" } {print "^holochain-", $1, ".", $2, ".[0-9]+$"}')
elif [ "$SEP_COUNT" == "2" ] && [[ "$VERSION_STR" =~ rc$ ]]; then
    # Matched format 0_X_rc
  SEARCH_PATTERN=$(echo "$VERSION_STR" | awk -F _ 'BEGIN { OFS="" } {print "^holochain-", $1, ".", $2, ".[0-9]+-rc.[0-9]+$"}')
elif [ "$VERSION_STR" == "weekly" ]; then
  # Special case, weekly tracks the latest pre-release version
  SEARCH_PATTERN="^holochain-[0-9]+.[0-9]+.[0-9]+-dev.[0-9]+$"
else
    echo "Invalid version format: $VERSION_STR"
    exit 1
fi

echo "Looking for tags matching pattern: $SEARCH_PATTERN"

LATEST_MATCHING_TAG=$(git tag --list --sort=version:refname | { grep -E "$SEARCH_PATTERN" || printf ""; } | tail -n 1)

if [ -z "$LATEST_MATCHING_TAG" ]; then
    echo "No matching tag found for: $VERSION_STR"
    exit 1
fi

echo "Latest matching tag: $LATEST_MATCHING_TAG"

sed --in-place "s#holochain/holochain/holochain-.*\"#holochain/holochain/$LATEST_MATCHING_TAG\"#" "./versions/$VERSION_STR/flake.nix"
