{ nixpkgs ? null
, flavor ? "coreDev"
, ... } @ args:

let
  default = import (builtins.toString ./default.nix) { inherit nixpkgs; };
in

builtins.getAttr flavor default.shells
