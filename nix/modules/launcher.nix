# Definitions can be imported from a separate file like this one

{ self, inputs, lib, ... }@flake: {
  perSystem = { config, self', inputs', system, pkgs, ... }:
    let
      rustToolchain = config.rustHelper.mkRust {
        track = "stable";
        version = "1.77.2";
      };

      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;

      crateInfo = craneLib.crateNameFromCargoToml { cargoToml = flake.config.reconciledInputs.launcher + "/crates/hc_launch/src-tauri/Cargo.toml"; };

      apple_sdk =
        if system == "x86_64-darwin"
        then pkgs.darwin.apple_sdk_10_12
        else pkgs.darwin.apple_sdk_11_0;

      commonArgs = {
        pname = "hc-launch";
        src = inputs.launcher;
        cargoExtraArgs = "--bin hc-launch";

        # We are asking Crane to build a binary from the workspace and that's the only way we can build it because
        # the workspace defines the dependencies, so we can't just build the member crate. But then we need to tell
        # Crate what the version is, so we look it up directly from the member's Cargo.toml.
        version = crateInfo.version;

        buildInputs = [
          pkgs.perl
        ]
        ++ (pkgs.lib.optionals pkgs.stdenv.isLinux
          [
            pkgs.glib
            pkgs.go
            pkgs.webkitgtk.dev
          ])
        ++ pkgs.lib.optionals pkgs.stdenv.isDarwin
          [
            apple_sdk.frameworks.AppKit
            apple_sdk.frameworks.WebKit

            (if pkgs.system == "x86_64-darwin" then
              pkgs.darwin.apple_sdk_11_0.stdenv.mkDerivation
                {
                  name = "go";
                  nativeBuildInputs = with pkgs; [
                    makeBinaryWrapper
                    go
                  ];
                  dontBuild = true;
                  dontUnpack = true;
                  installPhase = ''
                    makeWrapper ${pkgs.go}/bin/go $out/bin/go
                  '';
                }
            else pkgs.go)
          ]
        ;

        nativeBuildInputs = (
          if pkgs.stdenv.isLinux then [ pkgs.pkg-config ]
          else [ ]
        );

        doCheck = false;
      };

      # derivation building all dependencies
      deps = craneLib.buildDepsOnly
        (commonArgs // { });

      # derivation with the main crates
      hc-launch = craneLib.buildPackage
        (commonArgs // {
          cargoArtifacts = deps;

          stdenv =
            if pkgs.stdenv.isDarwin then
              pkgs.overrideSDK pkgs.stdenv "11.0"
            else
              pkgs.stdenv;
        });
    in
    {
      packages = {
        hc-launch = package;
      };
    };
}
