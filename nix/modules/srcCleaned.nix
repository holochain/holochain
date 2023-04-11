{ self, lib, inputs, ... }:

let
  includeCommon =
    [ "crates" "Cargo.toml" "Cargo.lock" "rustfmt.toml" "nextest.toml" ];

in
{
  options.srcCleanedRepo = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedRepo = inputs.nix-filter.lib {
    include = includeCommon;
    root = self;
  };

  options.srcCleanedRepoWithChangelogs = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedRepoWithChangelogs = inputs.nix-filter.lib {
    include = includeCommon ++ [
      (_args: path: _type: (builtins.match ".*/CHANGELOG.md" path) != null)
    ];
    root = self;
  };

  options.srcCleanedHolochain = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedHolochain = inputs.nix-filter.lib {
    include = includeCommon;
    root = inputs.holochain;
  };

  options.srcCleanedReleaseAutomationRepo = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedReleaseAutomationRepo = inputs.nix-filter.lib {
    include = includeCommon ++ [
      "src"
      "examples"
    ];
    root = "${self}/crates/release-automation";
  };

  options.srcCleanedDebugBuild = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedDebugBuild = inputs.nix-filter.lib {
    include = includeCommon ++ [
      "src"
    ];
    root = "${self}/crates/debug-build";
  };

  options.srcCleanedHolonix = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedHolonix = inputs.nix-filter.lib {
    include = [
      "holonix"
    ];
    root = self;
  };

}
