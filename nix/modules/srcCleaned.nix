{ self, lib, inputs, config, ... }@flake:

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
    root = flake.config.reconciledInputs.holochain;
  };

  options.srcCleanedReleaseAutomationRepo = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedReleaseAutomationRepo = inputs.nix-filter.lib {
    include = includeCommon ++ [
      "src"
      "examples"
    ];
    root = "${self}/crates/release-automation";
  };

  options.srcCleanedHolonix = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedHolonix = inputs.nix-filter.lib {
    include = [
      "holonix"
    ];
    root = self;
  };
}
