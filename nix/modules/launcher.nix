# Definitions can be imported from a separate file like this one

{ self, inputs, lib, ... }@flake: {
  perSystem = { config, self', inputs', system, pkgs, ... }:
    let
      rustToolchain = config.rustHelper.mkRust { };
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
          (with config.rustHelper.apple_sdk.frameworks; [
            AppKit
            CoreFoundation
            Foundation
            Security
            WebKit
            IOKit
          ])
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

      rustPkgs = config.rustHelper.mkRustPkgs { };

      cargoNix = config.rustHelper.mkCargoNix {
        name = "hc-launch-generated-crate2nix";
        src = inputs.launcher;
        pkgs = rustPkgs;
      };

    in
    {
      packages = {
        hc-launch = package;

        hc-launch-crate2nix =
          config.rustHelper.mkNoIfdPackage
            "hc-launch"
            cargoNix.workspaceMembers.holochain_cli_launch.build
        ;

        _debug-build-crate2nix =
          let
            cargoNix = config.rustHelper.mkCargoNix {
              name = "debug-build";
              src = flake.config.srcCleanedDebugBuild;
              pkgs = rustPkgs;
            };
          in
          cargoNix.allWorkspaceMembers;

        _debug-build =
          craneLib.buildPackage
            (lib.attrsets.recursiveUpdate commonArgs {
              cargoArtifacts = null;

              pname = "debug-build";
              cargoExtraArgs = "";
              CARGO_PROFILE = "release";
              src = flake.config.srcCleanedDebugBuild;
              doCheck = false;
            });
      };
    };
}





