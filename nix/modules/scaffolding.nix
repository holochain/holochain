# Definitions can be imported from a separate file like this one

{ self, inputs, lib, ... }@flake: {
  perSystem = { config, self', inputs', system, pkgs, ... }:
    let

      rustToolchain = config.rust.mkRust {
        track = "stable";
        version = "1.66.1";
      };
      craneLib = inputs.crane.lib.${system}.overrideToolchain rustToolchain;

      commonArgs = {

        pname = "hc-scaffold";
        src = flake.config.reconciledInputs.scaffolding;

        CARGO_PROFILE = "release";

        cargoExtraArgs = "--bin hc-scaffold";

        buildInputs =
          (with pkgs; [
            # TODO: remove sqlite package once https://github.com/holochain/holochain/pull/2248 is released
            openssl
            sqlite
          ])
          ++ (lib.optionals pkgs.stdenv.isDarwin
            (with pkgs.darwin.apple_sdk_11_0.frameworks; [
              AppKit
              CoreFoundation
              CoreServices
              Security
            ])
          );

        nativeBuildInputs =
          (with pkgs; [
            perl
            pkg-config
            makeBinaryWrapper
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

    in
    {
      packages = {
        hc-scaffold = package;
      };
    };
}
