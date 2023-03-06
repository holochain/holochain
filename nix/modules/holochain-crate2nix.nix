{ self, lib, inputs, ... } @ flake: {
  perSystem = { config, self', inputs', pkgs, system, ... }:
    let
      crate2nixTools = import "${inputs.crate2nix}/tools.nix" {
        inherit pkgs;
      };
      customBuildRustCrateForPkgs = pkgs: pkgs.buildRustCrate.override {
        defaultCrateOverrides = pkgs.defaultCrateOverrides // {
          buildInputs = (with pkgs; [ go ]) ++ (lib.optionals);
          nativeBuildInputs = (with pkgs; [ go ]) ++ (lib.optionals);
          holochain = attrs: {
            codegenUnits = 8;
          };
        };
      };
      generated = crate2nixTools.generatedCargoNix {
        name = "holochain-generated-crate2nix";
        src = flake.config.srcCleanedHolochain;
      };
      cargoNix = pkgs.callPackage "${generated}/default.nix" {
        buildRustCrateForPkgs = customBuildRustCrateForPkgs;
      };

      # `nix flake show` is incompatible with IFD by default
      # This works around the issue by making the name of the package
      #   discoverable without IFD.
      mkNoIfdPackage = name: pkg: {
        inherit name;
        inherit (pkg) drvPath outPath;
        type = "derivation";
        orig = pkg;
      };
    in
    {
      packages = {
        build-holochain-build-crates-standalone =
          mkNoIfdPackage "holochain" cargoNix.allWorkspaceMembers;
      };
    };
}
