{ callPackage, binaryen, lib }:
let node = callPackage ../node/default.nix { };
in lib.attrsets.recursiveUpdate node { buildInputs = [ binaryen ]; }
