{ self, lib, inputs, ... }: {
  options.srcCleaned = lib.mkOption {type = lib.types.raw;};
  config.srcCleaned = inputs.nix-filter.lib {
    root = self;
    # Works like include, but the reverse.
    exclude = [
      (inputs.nix-filter.lib.matchExt "nix")
    ];
  };
}
