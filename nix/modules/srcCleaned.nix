{ self, lib, inputs, ... }:

let
  includeCommon =
    [ "crates" "Cargo.toml" "Cargo.lock" "rustfmt.toml" "nextest.toml" ];

in
{
  options.srcCleaned = lib.mkOption { type = lib.types.raw; };
  config.srcCleaned = inputs.nix-filter.lib {
    include = includeCommon;
    root = self;
  };

  options.srcCleanedTests = lib.mkOption { type = lib.types.raw; };
  config.srcCleanedTests = inputs.nix-filter.lib {
    root = self;
    include = includeCommon ++ [ ".config" ];
  };
}
