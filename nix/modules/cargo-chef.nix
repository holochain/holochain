# Definitions can be imported from a separate file like this one

{ self, inputs, lib, ... }@flake: {
  perSystem = { config, self', inputs', system, pkgs, ... }:
    let
      rustToolchain = config.rust.mkRust {
        track = "stable";
        version = "latest";
      };

      craneLib = inputs.crane.lib.${system}.overrideToolchain rustToolchain;

      cargo-chef = craneLib.buildPackage {
        src = inputs.cargo-chef;
        doCheck = false;
      };

    in
    {
      packages = {
        inherit cargo-chef;
      };
    };
}
