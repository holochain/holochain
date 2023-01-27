# TODO: potentially repair or remove this. unused as of 2023-01-27 import-from-derivation is broken

{ callPackage, crate2nixSrc }:

let
  crate2nix-tools = callPackage "${crate2nixSrc}/tools.nix" { };
  generated = crate2nix-tools.generatedCargoNix {
    name = "release-automation";
    src = ./.;
  };
in callPackage "${generated}/default.nix" { }
