{ self, lib, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }:
    {
      # makes all pre-commit hooks become part of `nix flake check`
      pre-commit.check.enable = true;

      # enables the nixpkgs-fmt module for pre-commit-hooks
      pre-commit.settings.hooks.nixpkgs-fmt.enable = true;
    };
}
