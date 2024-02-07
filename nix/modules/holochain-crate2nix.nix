{ self, lib, inputs, ... } @ flake: {
  perSystem = { config, self', inputs', pkgs, system, ... }:
    let
      cargoNix = config.rustHelper.mkCargoNix {
        name = "holochain-generated-crate2nix";
        src = flake.config.srcCleanedHolochain;
        pkgs = config.rustHelper.mkRustPkgs {
          track = "stable";
          version = "1.75.0";
        };
      };
    in
    {
      packages = {
        build-holochain-build-crates-standalone =
          config.rustHelper.mkNoIfdPackage "holochain" cargoNix.allWorkspaceMembers;

        # exposed just for manual debugging
        holochain-crate2nix =
          config.rustHelper.mkNoIfdPackage "holochain" cargoNix.workspaceMembers.holochain.build;
      };
    };
}
