# Definitions can be imported from a separate file like this one

{ self, inputs, lib, ... }@flake: {
  perSystem = { config, self', inputs', system, pkgs, ... }:
    let
      rustToolchain = config.rustHelper.mkRust {
        track = "stable";
        version = "1.91.1";
      };

      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;

      commonArgs = {
        RUST_SODIUM_LIB_DIR = "${pkgs.libsodium}/lib";
        RUST_SODIUM_SHARED = "1";
        LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

        pname = "holochain";
        src = flake.config.srcCleanedHolochain;

        version = "workspace";

        CARGO_PROFILE = "";

        buildInputs = (with pkgs; [
          openssl
          self'.packages.opensslStatic
          sqlcipher
          cmake
        ]);

        nativeBuildInputs = (with pkgs; [
          makeWrapper
          perl
          pkg-config
          go
          # These packages and env vars are required to build holochain with the 'wasmer_wamr' feature 
          clang
          llvmPackages.libclang.lib
          ninja
        ])
        ++ lib.optionals pkgs.stdenv.isDarwin
          (with pkgs; [ xcbuild libiconv ]);

        stdenv = config.rustHelper.defaultStdenv pkgs;
      };

      # derivation building all dependencies
      holochainDeps = craneLib.buildDepsOnly (commonArgs // {
        src = flake.config.srcCleanedHolochain;
        doCheck = false;
      });

      holochainDepsRelease = craneLib.buildDepsOnly (commonArgs // {
        CARGO_PROFILE = "release";
        src = flake.config.srcCleanedHolochain;
        doCheck = false;
      });

      # derivation with the main crates
      holochain = lib.makeOverridable craneLib.buildPackage (commonArgs // {
        CARGO_PROFILE = "release";
        cargoArtifacts = holochainDepsRelease;
        src = flake.config.srcCleanedHolochain;
        doCheck = false;
        passthru.src.rev = flake.config.reconciledInputs.holochain.rev;
      });

    in
    {
      packages =
        {
          inherit
            holochain
            ;
        };
    };
}
