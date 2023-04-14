{ self, lib, inputs, ... }:

let
  includeCommon =
    [ "crates" "Cargo.toml" "Cargo.lock" "rustfmt.toml" "nextest.toml" ];
  excludeCommon = [ "crates/src/release-automation" ];
  excludeTests = _args: path: _type: (builtins.match "crates/.*/tests?" path) != null;

in
{
  options.srcCleanedRepo = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedRepo = inputs.nix-filter.lib {
    include = includeCommon;
    root = self;
    exclude = excludeCommon;
  };

  options.srcCleanedRepoNoTests = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedRepoNoTests = inputs.nix-filter.lib {
    include = includeCommon;
    root = self;
    exclude = excludeCommon ++ [
      excludeTests
    ];
  };

  options.srcCleanedHolochain = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedHolochain = inputs.nix-filter.lib {
    include = includeCommon;
    root = inputs.holochain;
    exclude = excludeCommon;
  };

  options.srcCleanedHolochainNoTests = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedHolochainNoTests = inputs.nix-filter.lib {
    include = includeCommon;
    root = inputs.holochain;
    exclude =
      excludeCommon ++ [
        excludeTests
      ];
  };

  options.srcCleanedReleaseAutomationRepo = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedReleaseAutomationRepo = inputs.nix-filter.lib {
    include = includeCommon ++ [
      "src"
      "examples"
      "tests"
    ];
    root = "${self}/crates/release-automation";
  };

  options.srcCleanedReleaseAutomationRepoNoTests = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedReleaseAutomationRepoNoTests = inputs.nix-filter.lib {
    include = includeCommon ++ [
      "src"
      "examples"
    ];
    root = "${self}/crates/release-automation";
    exclude = [
      "src/lib/tests"
    ];
  };

  options.srcCleanedHolonix = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedHolonix = inputs.nix-filter.lib {
    include = [
      "holonix"
    ];
    root = self;
  };
}
