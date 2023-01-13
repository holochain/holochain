{ self, lib, inputs, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }: {
    options.crate2nix = lib.mkOption {
      type = lib.types.package;
    };
    config.crate2nix = import inputs.crate2nix {
      inherit pkgs;
    };
  };
}
