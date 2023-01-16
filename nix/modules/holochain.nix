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
          (with pkgs.darwin.apple_sdk_11_0.frameworks; [
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
    holochainDeps = craneLib.buildDepsOnly (commonArgs // rec {
      RUST_SODIUM_LIB_DIR = "${pkgs.libsodium}/lib";
      RUST_SODIUM_SHARED = "1";
    });

    # derivation with the main crates
    holochain = craneLib.buildPackage (commonArgs // {
      cargoArtifacts = holochainDeps;
      doInstallCargoArtifacts = true;
      doCheck = false;
    });

    # derivation with the main crates
    holochain-tests = craneLib.buildPackage (commonArgs // {
      cargoArtifacts = holochainDeps;
      doCheck = true;
      cargoExtraArgs = "--features build_wasms";
    });

    holochain-tests-nextest = craneLib.cargoNextest (commonArgs // {
      cargoArtifacts = holochainDeps;
      preCheck = ''
        rm /build/source/target/debug/.fingerprint/holochain_wasm_test_utils-*/invoked.timestamp
        rm /build/source/target/debug/.fingerprint/holochain_test_wasm_common-*/invoked.timestamp
      '';
      # cargoExtraArgs = "--features slow_tests,glacial_tests,test_utils,build_wasms,db-encryption";
      # CARGO_PROFILE = "release";
      checkPhaseCargoCommand = ''
        cargo nextest run --workspace --features slow_tests,glacial_tests,test_utils,build_wasms,db-encryption --lib --tests
      '';
    });

  in {
    packages = {inherit holochain holochain-tests holochain-tests-nextest;};
  };
}
