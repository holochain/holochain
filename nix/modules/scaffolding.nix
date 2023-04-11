# Definitions can be imported from a separate file like this one

{ self, inputs, lib, ... }@flake: {
  perSystem = { config, self', inputs', system, pkgs, ... }:
    let
      rustToolchain = config.rustHelper.mkRust { };
      craneLib = inputs.crane.lib.${system}.overrideToolchain rustToolchain;

      commonArgs = {

        pname = "hc-scaffold";
        src = inputs.scaffolding;

        CARGO_PROFILE = "release";

        cargoExtraArgs = "--bin hc-scaffold";

        buildInputs =
          (with pkgs; [
            openssl

            # TODO: remove sqlite package once https://github.com/holochain/holochain/pull/2248 is released
            sqlite
          ]) ++ (lib.optionals pkgs.stdenv.isDarwin
            (with config.rustHelper.apple_sdk.frameworks; [
              AppKit
              Foundation
              Security
              WebKit
            ])
          )
        ;

        nativeBuildInputs =
          (with pkgs; [
            perl
            pkg-config
            makeBinaryWrapper
            self'.packages.goWrapper
          ])
          ++ lib.optionals pkgs.stdenv.isDarwin (with pkgs; [
            xcbuild
            libiconv
          ]);

        doCheck = false;
      };

      # derivation building all dependencies
      deps = craneLib.buildDepsOnly (commonArgs // { });

      # derivation with the main crates
      package = craneLib.buildPackage (commonArgs // {
        cargoArtifacts = deps;

        preFixup = ''
          wrapProgram $out/bin/hc-scaffold \
            --prefix PATH : ${rustToolchain}/bin
        '';
      });

      rustPkgs = config.rustHelper.mkRustPkgs {
        track = "stable";
        version = "1.69.0";
      };

      cargoNix = config.rustHelper.mkCargoNix {
        name = "hc-scaffold-generated-crate2nix";
        src = inputs.scaffolding;
        pkgs = rustPkgs;
      };

    in
    {
      packages = {
        hc-scaffold = package;

        hc-scaffold-crate2nix =
          config.rustHelper.mkNoIfdPackage
            "hc-scaffold"
            (cargoNix.workspaceMembers.holochain_scaffolding_cli.build.overrideAttrs
              (attrs: {
                preFixup = ''
                  wrapProgram $out/bin/hc-scaffold \
                    --prefix PATH : ${rustPkgs.cargo}/bin
                '';
              }));
      };
    };
}
