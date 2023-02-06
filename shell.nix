{ flavor ? null
, flavour ? null
, rustVersion ? null
, ...
} @ args:

let
  flavor' =
    if flavor != null then flavor
    else if flavour != null then flavour
    else "coreDev";
  default = import (builtins.toString ./default.nix) (
    if rustVersion != null then {
      inherit rustVersion;
    } else { }
      // (builtins.removeAttrs args [
      "flavor"
      "flavour"
      "inNixShell"
    ])
  );
in

builtins.getAttr flavor' default.shells
