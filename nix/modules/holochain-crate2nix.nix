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
  in {
    packages.holochain-crate2nix = holochain.override {
      # runTests = true;
    };
  };
}
