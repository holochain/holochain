#!/bin/sh
nix eval --impure --raw --expr '(import (import ./nix/sources.nix {}).holochain-nixpkgs {}).pkgs.path'
