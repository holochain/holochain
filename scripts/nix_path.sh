#!/bin/sh
# TODO: this is broken
nix eval --impure --raw --expr '(import (import ./nix/sources.nix {}).holonix {}).pkgs.path'
