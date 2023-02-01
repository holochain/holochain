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

        pname = "launcher";
        src = inputs.launcher;

        CARGO_PROFILE = "release";

        cargoExtraArgs = "--bin hc-launch";

        buildInputs =
          (with pkgs; [
            openssl
            glib
          ])
          ++ (lib.optionals pkgs.stdenv.isLinux
            (with pkgs; [
              webkitgtk.dev
              gdk-pixbuf
              gtk3
            ]))
          ++ (lib.optionals pkgs.stdenv.isDarwin
            (with pkgs.darwin.apple_sdk_11_0.frameworks; [
              AppKit
              CoreFoundation
              CoreServices
              Security
              WebKit
            ])
          );

        nativeBuildInputs =
          (with pkgs;
          [
            perl
            pkg-config
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
      });

    in
    {
      packages = {
        launcher = package;
      };
    };
}
