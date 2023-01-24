# Example: mkShell based environment without NodeJS
# 
# The following `shell.nix` file can be used in your project's root folder and activated with `nix-shell`.
# It demonstrates how to exclude _node_ and _happs_ (that includes the node toolchain as well) components.

{ holonixPath ? builtins.fetchTarball {
  url = "https://github.com/holochain/holonix/archive/develop.tar.gz";
} }:

let
  holonix = import (holonixPath) {
    holochainVersionId = "develop";

    include = {
      holochainBinaries = true;
      node = false;
      happs = false;
    };
  };
  nixpkgs = holonix.pkgs;

in nixpkgs.mkShell {
  inputsFrom = [ holonix.main ];
  buildInputs = with nixpkgs; [ binaryen ];
}
