{ self, lib, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }:
    {
      pre-commit.check.enable = true;
      pre-commit.settings.hooks.nixpkgs-fmt.enable = true;
    };
}
