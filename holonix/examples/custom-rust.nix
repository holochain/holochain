# Example: mkShell based environment with a custom rust toolchain

{ holonixPath ? builtins.fetchTarball {
  url = "https://github.com/holochain/holonix/archive/develop.tar.gz";
} }:

let
  holonix = import (holonixPath) {
    rustVersion = {
      track = "stable";
      version = "1.61.0";
    };
  };

  nixpkgs = holonix.pkgs;

in nixpkgs.mkShell {
  inputsFrom = [ holonix.main ];
  buildInputs = with nixpkgs;
    [
      # custom packages go here
    ];
}
