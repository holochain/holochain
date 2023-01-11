{ self, lib, inputs, ... }: {
  perSystem = { config, self', inputs', pkgs, system, ... }: {
    options.rust = lib.mkOption {
      type = lib.types.raw;
    };
    config.rust = let
      rustPkgs = import inputs.nixpkgs {
        inherit system;
        overlays = [
          inputs.rust-overlay.overlays.default
          (import ./overlay.nix)
        ];
      };
    in
      rustPkgs.rust;
  };
}
