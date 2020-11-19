{ nixpkgs ? import <nixpkgs> {}
, flavor ? "coreDevRustup"
, ... } @ args:

let
  default = import (builtins.toString ./default.nix) { inherit nixpkgs; };
in

builtins.getAttr flavor default.shells
