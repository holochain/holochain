#!/bin/sh
echo warning: this script has moved to a nix script, running said nix script..
set -xe
exec nix run .#scripts-ci-generate-readmes