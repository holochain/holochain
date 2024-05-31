# Definitions can be imported from a separate file like this one

{ self, inputs, lib, ... }@flake: {
  perSystem = { config, self', inputs', system, pkgs, ... }:
    let
      rustToolchain = config.rustHelper.mkRust {
        track = "stable";
        version = "1.77.2";
      };

      craneLib = inputs.crane.lib.${system}.overrideToolchain rustToolchain;

      crateInfo = craneLib.crateNameFromCargoToml { cargoToml = flake.config.reconciledInputs.launcher + "/crates/hc_launch/src-tauri/Cargo.toml"; };

      commonArgs = {

        pname = "hc-launch";
        src = flake.config.reconciledInputs.launcher;

        # We are asking Crane to build a binary from the workspace and that's the only way we can build it because
        # the workspace defines the dependencies, so we can't just build the member crate. But then we need to tell
        # Crate what the version is, so we look it up directly from the member's Cargo.toml.
        version = crateInfo.version;

        CARGO_PROFILE = "release";

        cargoExtraArgs = "--bin hc-launch";

        buildInputs = (with pkgs; [
          openssl

          # this is required for glib-networking
          glib
        ])
        ++ (lib.optionals pkgs.stdenv.isLinux
          (with pkgs; [
            webkitgtk.dev
            gdk-pixbuf
            gtk3
          ]))
        ++ lib.optionals pkgs.stdenv.isDarwin
          (with pkgs.darwin.apple_sdk_11_0.frameworks; [
            AppKit
            CoreFoundation
            CoreServices
            Security
            IOKit
            WebKit
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

    in
    {
      packages = {
        hc-launch = package;
      };
    };
}
