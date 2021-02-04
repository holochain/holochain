{ nixpkgs ? null }:

# This is an example of what downstream consumers of holonix should do
# This is also used to dogfood as many commands as possible for holonix
# For example the release process for holonix uses this file
let
  # point this to your local config.nix file for this project
  # example.config.nix shows and documents a lot of the options
  config = import ./config.nix;

  # START HOLONIX IMPORT BOILERPLATE
  holonixPath = if ! config.holonix.use-github
   then config.holonix.local.path
   else fetchTarball {
    url = "https://github.com/${config.holonix.github.owner}/${config.holonix.github.repo}/tarball/${config.holonix.github.ref}";
    sha256 = config.holonix.github.sha256;
   }
   ;
  holonix = import (holonixPath) {
    inherit config;
    includeHolochainBinaries = config.includeHolochainBinaries or false;
  };
  # END HOLONIX IMPORT BOILERPLATE

  overlays = [
    (self: super: {
      inherit holonix holonixPath;

      hcToplevelDir = builtins.toString ./.;

      nixEnvPrefixEval = ''
        if [[ -n "$NIX_ENV_PREFIX" ]]; then
          # don't touch it
          :
        elif test -d "${builtins.toString self.hcToplevelDir}" &&
            test -w "${builtins.toString self.hcToplevelDir}"; then
          export NIX_ENV_PREFIX="${builtins.toString self.hcToplevelDir}"
        elif test -d "$HOME" && test -w "$HOME"; then
          export NIX_ENV_PREFIX="$HOME/.cache/holochain-dev"
          mkdir -p "$NIX_ENV_PREFIX"
        else
          export NIX_ENV_PREFIX="$(${self.coreutils}/bin/mktemp -d)"
        fi
      '';

      inherit (holonix.pkgs.callPackage ./nix/rust.nix { }) hcRustPlatform;
    })
  ];

  nixpkgs' = import (nixpkgs.path or holonix.pkgs.path) { inherit overlays; };
  inherit (nixpkgs') callPackage;

  pkgs = callPackage ./nix/pkgs/default.nix { };
in
{
  inherit
    holonix
    pkgs
    ;

  # TODO: refactor when we start releasing again
  # releaseHooks = callPackages ./nix/release {
  #   inherit
  #     config
  #     nixpkgs
  #     ;
  # };

  shells = callPackage ./nix/shells.nix {
    inherit
      holonix
      pkgs
      ;
  };
}
