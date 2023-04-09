# Definitions can be imported from a separate file like this one

{ self, inputs, lib, ... }@flake: {
  perSystem = { config, self', inputs', system, pkgs, ... }:
    let
      rustToolchain = config.rust.mkRust {
        track = "stable";
        version = "latest";
      };

      craneLib = inputs.crane.lib.${system}.overrideToolchain rustToolchain;

      cargo-rdme = craneLib.buildPackage {
        src = inputs.cargo-rdme;
        doCheck = false;
      };

    in
    {
      packages = {
        inherit cargo-rdme;
      };
    };
}
