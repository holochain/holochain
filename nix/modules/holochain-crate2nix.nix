{ self, lib, inputs, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }: let
    crate2nixTools = import "${inputs.crate2nix}/tools.nix" {
      inherit pkgs;
    };
    # TODO: Tests are not running despite the overrides
    customBuildRustCrateForPkgs = pkgs: pkgs.buildRustCrate.override {
      defaultCrateOverrides = pkgs.defaultCrateOverrides // {
        holochain = attrs: {
          tunTests = true;
        };
      };
    };
    generated = crate2nixTools.generatedCargoNix {
      name = "holochain-generated-crate2nix";
      src = self.srcCleaned;
    };
    called = pkgs.callPackage "${generated}/default.nix" {
      buildRustCrateForPkgs = customBuildRustCrateForPkgs;
    };
    holochain = called.workspaceMembers.holochain.build;

    # `nix flake show` is incompatible with IFD by default
    # This works around the issue by making the name of the package
    #   discoverable without IFD.
    mkNoIfdPackage = name: pkg: {
      inherit name;
      inherit (pkg.holochain) drvPath outPath;
      type = "derivation";
    };
  in {
    packages.holochain-crate2nix = mkNoIfdPackage "holochain" holochain;
  };
}
