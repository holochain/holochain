{ self, lib, ... } @ flake: {
  perSystem = { config, self', inputs', pkgs, system, ... }: {
    options.pkgs = lib.mkOption {
      type = lib.types.raw;
    };
    config.pkgs = import flake.config.sources.nixpkgs {
      inherit system;
    };
  };
}
