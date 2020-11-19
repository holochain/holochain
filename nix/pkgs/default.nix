{ callPackage
, hcRustPlatform
, holonixPath
}:

let
  pkgs = {
    applications = callPackage ./applications.nix { inherit hcRustPlatform; };
    dev = callPackage ./dev.nix { inherit holonixPath; };
  };
in

builtins.mapAttrs (k: v:
  builtins.removeAttrs v [ "override" "overrideDerivation" ]
) pkgs
