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

        pname = "hc-launch";
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
          ++ (lib.optionals pkgs.stdenv.isDarwin (with pkgs.darwin.apple_sdk.frameworks; [
            AppKit
            CoreFoundation
            CoreServices
            Security
            IOKit
            WebKit
          ]));


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

        nativeBuildInputs = commonArgs.nativeBuildInputs ++ [
          pkgs.makeBinaryWrapper
        ];

        preFixup = ''
          wrapProgram $out/bin/hc-launch \
            --set WEBKIT_DISABLE_COMPOSITING_MODE 1 \
            --set GIO_MODULE_DIR ${pkgs.glib-networking}/lib/gio/modules \
            --prefix GIO_EXTRA_MODULES : ${pkgs.glib-networking}/lib/gio/modules
        '';
      });

    in
    {
      packages = {
        hc-launch = package;
        launcherDeps = deps;
      };
    };
}
