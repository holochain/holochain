{ nixpkgs ? null
, rustVersion ? {
    track = "stable";
    version = "1.66.0";
  }

, holonixArgs ? { }
}:

# This is an example of what downstream consumers of holonix should do
# This is also used to dogfood as many commands as possible for holonix
# For example the release process for holonix uses this file
let
  # point this to your local config.nix file for this project
  # example.config.nix shows and documents a lot of the options
  config = import ./config.nix;
  sources = import ./nix/sources.nix;

  # START HOLONIX IMPORT BOILERPLATE
  holonixPath = ./holonix;
  holonix = config.holonix.importFn ({ inherit rustVersion; } // holonixArgs);
  # END HOLONIX IMPORT BOILERPLATE

  overlays = [
    (self: super: {
      inherit holonix holonixPath;

      hcToplevelDir = builtins.toString ./.;

      nixEnvPrefixEval = ''
        if [[ -n "$NIX_ENV_PREFIX" ]]; then
          # don't touch it
          :
        elif test -w "$PWD"; then
          export NIX_ENV_PREFIX="$PWD"
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

      rustPlatform = self.makeRustPlatform {
        rustc = holonix.pkgs.custom_rustc;
        cargo = holonix.pkgs.custom_rustc;
      };

      inherit (self.rustPlatform.rust) rustc cargo;

      cargo-nextest = self.rustPlatform.buildRustPackage {
        name = "cargo-nextest";

        src = sources.nextest.outPath;
        cargoSha256 = "sha256-E25P/vasIBQp4m3zGii7ZotzJ7b2kT6ma9glvmQXcnM=";

        cargoTestFlags = [
          # TODO: investigate some more why these tests fail in nix
          "--"
          "--skip=tests_integration::test_relocated_run"
          "--skip=tests_integration::test_run"
          "--skip=tests_integration::test_run_after_build"
        ];
      };
    })

  ]
  ++ [(
    self: super: {
      inherit crate2nix;
    }
  )];

  crate2nix = (import (nixpkgs.path or holonix.pkgs.path) {}).crate2nix;
  nixpkgs' = import (nixpkgs.path or holonix.pkgs.path) { inherit overlays; };
  inherit (nixpkgs') callPackage;

  pkgs = callPackage ./nix/pkgs/default.nix { };
in
{
  inherit
    nixpkgs'
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
