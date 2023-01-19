{ self, lib, inputs, ... } @ flake: {
  perSystem = { config, self', inputs', system, ... }: {
    options.rust = lib.mkOption {
      type = lib.types.raw;
    };
    config.rust = let
      rustPkgs = import config.pkgs.path {
        inherit system;
        overlays = [
          inputs.rust-overlay.overlays.default
          (import "${flake.config.sources.holochain-nixpkgs}/overlays/rust.nix")
        ];
      };
    in
      rustPkgs.rust;
  };
}
