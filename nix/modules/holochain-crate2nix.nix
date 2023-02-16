{ self, lib, inputs, ...  } @ flake: {
  perSystem = { config, self', inputs', pkgs, ... }: let
    crate2nixTools = import "${inputs.crate2nix}/tools.nix" {
      inherit pkgs;
    };
    # TODO: Tests are not running despite the overrides
    customBuildRustCrateForPkgs = pkgs: pkgs.buildRustCrate.override {
      defaultCrateOverrides = pkgs.defaultCrateOverrides // {
        holochain = attrs: {
          codegenUnits = 16;
        };
      };
    };
    generated = crate2nixTools.generatedCargoNix {
      name = "holochain-generated-crate2nix";
      src = flake.config.srcCleanedHolochain;
    };
    called = pkgs.callPackage "${generated}/default.nix" {
      buildRustCrateForPkgs = customBuildRustCrateForPkgs;
    };
    holochain = called.workspaceMembers.holochain.build.override {
      # buildTests = true;
      # runTests = true;

      # Setting the features leads to errors like:
      #   (building openssl-sys):
      #     thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: Os { code: 2, kind: NotFound, message: "No such file or directory" }', src/lib.rs:515:32
      # features = [
      #   "slow_tests"
      #   "glacial_tests"
      #   "test_utils"
      #   "build_wasms"
      #   "db-encryption"
      # ];

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
  in {
    packages = {
      holochain-crate2nix = mkNoIfdPackage "holochain" holochain;
      crate2nix = import inputs.crate2nix {inherit pkgs;};
    };
  };
}
