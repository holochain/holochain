{ self, lib, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }: {
    # define formatter used by `nix fmt`
    formatter = pkgs.nixpkgs-fmt;
  };
}
