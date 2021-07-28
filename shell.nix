{ flavor ? null
, flavour ? null
, ... } @ args:

let
  flavor' =
    if flavor != null then flavor
    else if flavour != null then flavour
    else "coreDev";
  default = import (builtins.toString ./default.nix) (builtins.removeAttrs args [
    "flavor"
    "flavour"
    "inNixShell"
  ]);
in

builtins.getAttr flavor' default.shells
