# Definitions can be imported from a separate file like this one

{ self, inputs, lib, ... }: {
  perSystem = { config, self', inputs', pkgs, system, ... }: let

    rustToolchain = config.rust.rustHolochain;
    craneLib = inputs.crane.lib.${system}.overrideToolchain rustToolchain;

    opensslStatic = pkgs.pkgsStatic.openssl;

    commonArgs = {

      pname = "holochain";
      src = self.srcCleaned;

      CARGO_PROFILE = "";

      OPENSSL_NO_VENDOR = "1";
      OPENSSL_LIB_DIR = "${opensslStatic.out}/lib";
      OPENSSL_INCLUDE_DIR = "${opensslStatic.dev}/include";

      buildInputs =
        (with pkgs; [
          openssl
          opensslStatic
          sqlcipher
        ])
        ++ (lib.optionals pkgs.stdenv.isDarwin
          (with pkgs.darwin.apple_sdk.frameworks; [
            AppKit
            CoreFoundation
            CoreServices
            Security
          ])
        );

      nativeBuildInputs =
        (with pkgs; [
          makeWrapper
          perl
          pkg-config
        ])
        ++ lib.optionals pkgs.stdenv.isDarwin (with pkgs; [ xcbuild libiconv ]);
    };

    # derivation building all dependencies
    cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
      RUST_SODIUM_LIB_DIR = "${pkgs.libsodium}/lib";
      RUST_SODIUM_SHARED = "1";
    });

    # derivation with the main crates
    holochain = craneLib.buildPackage (commonArgs // {
      inherit cargoArtifacts;
      doInstallCargoArtifacts = true;
      doCheck = false;
      # cargoExtraArgs = "--features 'build' -p holochain_wasm_test_utils";
    });

    holochain-tests = craneLib.cargoNextest (commonArgs // {
      cargoArtifacts = cargoArtifacts;
      preBuild = ''
        chmod -R +wx /build/source/target
      '';
      cargoExtraArgs = "--features slow_tests,glacial_tests,test_utils,build_wasms,db-encryption";
      # CARGO_PROFILE = "release";
      # checkPhase = ''
      #   cargo nextest run
      #   # cargo nextest run --workspace --features slow_tests,glacial_tests,test_utils,build_wasms,db-encryption --lib --tests --cargo-profile fast-test
      # '';
    });

  in {
    packages = {inherit holochain holochain-tests;};
  };
}
