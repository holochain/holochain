{ self, inputs, lib, ... }@flake: {
  perSystem = { config, self', inputs', system, pkgs, ... }:
    # { packages = { my_jq_new = pkgs.jq; }; };
    let
      rustToolchain = config.rustHelper.mkRust {
        track = "stable";
        version = "1.75.0";
        targets = [ "x86_64-unknown-linux-musl" ];
      };

      craneLib = inputs.crane.lib.${system}.overrideToolchain rustToolchain;

      crateInfo = craneLib.crateNameFromCargoToml { cargoToml = flake.config.reconciledInputs.holochain + "/crates/holochain/Cargo.toml"; };

      commonArgs = {
        RUST_SODIUM_LIB_DIR = "${pkgs.libsodium}/lib";
        RUST_SODIUM_SHARED = "1";

        pname = "holochain_portable";
        src = flake.config.srcCleanedHolochain;
        version = crateInfo.version;

        CARGO_BUILD_TARGET = "x86_64-unknown-linux-musl";
        # CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
        # cargoExtraArgs = "--target x86_64-unknown-linux-musl -C target-feature=+crt-static";

        nativeBuildInputs = (with pkgs; [ musl makeWrapper perl pkg-config self'.packages.goWrapper ]);

        # stdenv = config.rustHelper.defaultStdenv pkgs;
      };

      # derivation building all dependencies
      holochainPortableDepsRelease = craneLib.buildDepsOnly (commonArgs // {
        src = flake.config.srcCleanedHolochain;
        doCheck = false;
      });

      # derivation with the main crates
      holochain_portable = craneLib.buildPackage (commonArgs // {
        cargoArtifacts = holochainPortableDepsRelease;
        src = flake.config.srcCleanedHolochain;
        doCheck = false;
        # CARGO_BUILD_TARGET = "x86_64-unknown-linux-musl";
        # CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
        passthru.src.rev = flake.config.reconciledInputs.holochain.rev;
      });
    in
    {
      packages =
        {
          inherit
            holochain_portable
            ;
        };
    };
}
