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

        buildInputs = (with pkgs; [
          openssl

          # TODO: remove this once the features have been rearranged to use vendored sqlite
          sqlite
        ])
        ++ (lib.optionals pkgs.stdenv.isLinux
          (with pkgs; [
            webkitgtk.dev
            gdk-pixbuf
            gtk3
          ]))
        ++ lib.optionals pkgs.stdenv.isDarwin
          (
            let
              frameworks =
                lib.attrsets.optionalAttrs pkgs.stdenv.isDarwin
                  (
                    # aarch64 only uses 11.0 and x86_64 mixes them
                    if system == "x86_64-darwin"
                    then pkgs.darwin.apple_sdk_10_12
                    else pkgs.darwin.apple_sdk_11_0
                  ).frameworks;
            in
            (with frameworks; [
              AppKit
              CoreFoundation
              Foundation
              Security
              WebKit
              IOKit
            ])
          )
        ;

        nativeBuildInputs = (with pkgs; [
          perl
          pkg-config

          # currently needed to build tx5
          self'.packages.goWrapper
        ])
        ++ (lib.optionals pkgs.stdenv.isLinux
          (with pkgs; [
            wrapGAppsHook
          ]))
        ++ (lib.optionals pkgs.stdenv.isDarwin [
          pkgs.xcbuild
          pkgs.libiconv
        ])
        ;

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
          gappsWrapperArgs+=(
            --set WEBKIT_DISABLE_COMPOSITING_MODE 1
          )

          # without this the DevTools will just display an unparsed HTML file (see https://github.com/tauri-apps/tauri/issues/5711#issuecomment-1336409601)
          gappsWrapperArgs+=(
            --prefix XDG_DATA_DIRS : "${pkgs.shared-mime-info}/share"
          )
        '';
      });

    in
    {
      packages = {
        hc-launch = package;
      };
    };
}





