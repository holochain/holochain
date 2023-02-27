{ self, lib, inputs, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }: {
    packages.crate2nix = pkgs.callPackage inputs.crate2nix;
  };
}
