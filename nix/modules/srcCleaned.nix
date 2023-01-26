{ self, lib, inputs, ... }: {
  options.srcCleaned = lib.mkOption { type = lib.types.raw; };
  config.srcCleaned = inputs.nix-filter.lib {
    root = self;
    # Works like include, but the reverse.
    include =
      [ "crates" "Cargo.toml" "Cargo.lock" "rustfmt.toml" "nextest.toml" ];
  };
}
