# Definitions can be imported from a separate file like this one

{ self, inputs, lib, ... }@flake: {
  perSystem = { config, self', inputs', system, pkgs, ... }:
    let
      rustToolchain = config.rustHelper.mkRust {
        track = "stable";
        version = "1.77.2";
      };

      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;

      crateInfo = craneLib.crateNameFromCargoToml { cargoToml = flake.config.reconciledInputs.lair + "/crates/lair_keystore/Cargo.toml"; };

      commonArgs = {

        pname = "lair-keystore";
        src = flake.config.reconciledInputs.lair;

        # We are asking Crane to build a binary from the workspace and that's the only way we can build it because
        # the workspace defines the dependencies, so we can't just build the member crate. But then we need to tell
        # Crate what the version is, so we look it up directly from the member's Cargo.toml.
        version = crateInfo.version;

        CARGO_PROFILE = "release";

        cargoExtraArgs = "--bin lair-keystore";

        buildInputs = (with pkgs; [ openssl ])
          ++ (lib.optionals pkgs.stdenv.isDarwin
          (with pkgs.darwin.apple_sdk_11_0.frameworks; [
            AppKit
            CoreFoundation
            CoreServices
            Security
          ]));

        nativeBuildInputs = (with pkgs; [ perl pkg-config ])
          ++ lib.optionals pkgs.stdenv.isDarwin
          (with pkgs; [ xcbuild libiconv ]);

        doCheck = false;
      };

      # derivation building all dependencies
      deps = craneLib.buildDepsOnly (commonArgs // { });

      # derivation with the main crates
      package = lib.makeOverridable craneLib.buildPackage (commonArgs // {
        cargoArtifacts = deps;
      });

    in
    { packages = { lair-keystore = package; }; };
}
