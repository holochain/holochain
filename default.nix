{ flavor ? null
, flavour ? null
, rustVersion ? null
, ... }:

let
  flavor' =
    if flavor != null then flavor
    else if flavour != null then flavour
    else "default";

  flake = (import ./nix/compat.nix);
in

flake.devShells.${builtins.currentSystem}.${flavor'}
  or flake.devShells.${builtins.currentSystem}.default
