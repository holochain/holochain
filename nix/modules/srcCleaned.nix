{ self, lib, inputs, config, ... }@flake:

let
  # Filter out files or folders with this infix
  matchInfix = infix:
    root: path: type:
      lib.strings.hasInfix infix path;

  includeCommon =
    [ "crates" "Cargo.toml" "Cargo.lock" "rustfmt.toml" "nextest.toml" ];
  excludeCommon = [
    (inputs.nix-filter.lib.matchExt "md")
    (matchInfix "LICENSE")
    (inputs.nix-filter.lib.matchName ".gitignore")
  ];

in
{
  options.srcCleanedRepo = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedRepo = inputs.nix-filter.lib {
    include = includeCommon;
    exclude = excludeCommon;
    root = self;
  };

  options.srcCleanedHolochain = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedHolochain = inputs.nix-filter.lib {
    include = includeCommon;
    exclude = excludeCommon;
    root = flake.config.reconciledInputs.holochain;
  };

  options.srcCleanedReleaseAutomationRepo = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedReleaseAutomationRepo = inputs.nix-filter.lib {
    include = includeCommon ++ [
      "src"
      "examples"
    ];
    exclude = excludeCommon ++
      [
        "src/lib/tests"
        "src/lib/crate_selection/tests"
      ];
    root = "${self}/crates/release-automation";
  };

  options.srcCleanedReleaseAutomationWithTestsRepo = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedReleaseAutomationWithTestsRepo = inputs.nix-filter.lib {
    include = includeCommon ++ [
      "src"
      "examples"
    ];
    exclude = excludeCommon;
    root = "${self}/crates/release-automation";
  };

  options.srcCleanedHolonix = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedHolonix = inputs.nix-filter.lib {
    include = [
      "holonix"
    ];
    exclude = excludeCommon;
    root = self;
  };
}
