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

  options.srcCleanedHolochain = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedHolochain = inputs.nix-filter.lib {
    include = includeCommon;
    root = inputs.holonix.inputs.holochain;
  };

  options.srcCleanedReleaseAutomationRepo = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedReleaseAutomationRepo = inputs.nix-filter.lib {
    include = includeCommon ++ [
      "src"
      "examples"
    ];
    root = "${self}/crates/release-automation";
  };
}
