#!/usr/bin/env bash

set -euo pipefail

count=$1
shift

# If the command fails or the script is interrupted then print progress and exit.
trap 'echo "Aborted after $i/$count iterations"; exit 1' ERR SIGINT

for i in $(seq "$count"); do
    echo "Running $i/$count"
    "$@"
done

echo "Finished $count iterations"
