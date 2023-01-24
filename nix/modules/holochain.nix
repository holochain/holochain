# Definitions can be imported from a separate file like this one

{ self, inputs, lib, ... } @ flake: {
  perSystem = { config, self', inputs', system, ... }: let

    pkgs = config.pkgs;

    rustToolchain = config.rust.rustHolochain;
    craneLib = inputs.crane.lib.${system}.overrideToolchain rustToolchain;

    opensslStatic = pkgs.pkgsStatic.openssl;

    commonArgs = {

      pname = "holochain";
      src = flake.config.srcCleaned;

      version = let
        holochainCargoToml = builtins.fromTOML (
          builtins.readFile (self + /crates/holochain/Cargo.toml)
        );
      in
        holochainCargoToml.package.version;

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
      doCheck = false;
    });

    holochainTestDeps = craneLib.buildDepsOnly (commonArgs // rec {
      RUST_SODIUM_LIB_DIR = "${pkgs.libsodium}/lib";
      RUST_SODIUM_SHARED = "1";
      pname = "holochain-tests";
      CARGO_PROFILE = "fast-test";
      # cargoExtraArgs = ''
      #   --features slow_tests,glacial_tests,test_utils,build_wasms,db-encryption --lib --tests
      # '';
    });

    disabledTests = [
      # "conductor::cell::gossip_test::gossip_test"
      # "conductor::interface::websocket::test::enable_disable_enable_app"
      # "conductor::interface::websocket::test::websocket_call_zome_function"
      # "core::ribosome::host_fn::accept_countersigning_preflight_request::wasm_test::enzymatic_session_fail"
      # "core::ribosome::host_fn::remote_signal::tests::remote_signal_test"
      # "core::workflow::app_validation_workflow::tests::app_validation_workflow_test"
      # "core::workflow::app_validation_workflow::validation_tests::app_validation_ops"
      # "core::workflow::sys_validation_workflow::tests::sys_validation_workflow_test"
      # "local_network_tests::conductors_call_remote::_2"
      # "local_network_tests::conductors_call_remote::_4"
      # "conductor::interface::websocket::test::enable_disable_enable_apped"
    ];

    disabledTestsArgs =
      lib.forEach disabledTests (test: "-E 'not test(${test})'");


    holochain-tests-nextest = craneLib.cargoNextest (commonArgs // {
      __noChroot = true;
      cargoArtifacts = holochainTestDeps;
      preCheck = ''
        pwd
        # rm /build/source/target/debug/.fingerprint/holochain_wasm_test_utils-*/invoked.timestamp
        # rm /build/source/target/debug/.fingerprint/holochain_test_wasm_common-*/invoked.timestamp
      '';
      # cargoExtraArgs = "--features slow_tests,glacial_tests,test_utils,build_wasms,db-encryption";
      # CARGO_PROFILE = "release";
      cargoExtraArgs = ''
        --test-threads 2 --workspace --features slow_tests,glacial_tests,test_utils,build_wasms,db-encryption --lib --tests --cargo-profile fast-test \
        ${lib.concatStringsSep " " disabledTestsArgs}
      '';

      dontPatchELF = true;

      # cargoNextestExtraArgs = lib.concatStringsSep " " disabledTestsArgs;
    });

  in {
    packages = {inherit holochain holochain-tests-nextest;};
  };
}
