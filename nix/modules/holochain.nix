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
          (lib.attrValues
          (lib.filterAttrs (n: _: ! lib.elem n [
            "QuickTime"
          ]) pkgs.darwin.apple_sdk_11_0.frameworks))
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
      cargoExtraArgs = ''--features slow_tests,glacial_tests,test_utils,build_wasms,db-encryption --lib --tests'';
    });

    holochainNextestDeps = craneLib.buildDepsOnly (commonArgs // rec {
      RUST_SODIUM_LIB_DIR = "${pkgs.libsodium}/lib";
      RUST_SODIUM_SHARED = "1";
      pname = "holochain-nextest";
      CARGO_PROFILE = "fast-test";
      cargoExtraArgs = ''--features slow_tests,glacial_tests,test_utils,build_wasms,db-encryption --lib --tests'';
      nativeBuildInputs = [
        pkgs.cargo-nextest
      ];
      buildPhase = ''
        cargo nextest list ${import ../../.config/nextest-args.nix}
      '';
      dontCheck = true;
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

    holochain-tests = craneLib.cargoTest (commonArgs // {
      pname = "holochain";
      __impure = pkgs.stdenv.isLinux;
      cargoArtifacts = holochainTestDeps;
      CARGO_PROFILE = "fast-test";
      cargoExtraArgs = ''--features slow_tests,glacial_tests,test_utils,build_wasms,db-encryption --lib --tests'';

      dontPatchELF = true;
      dontFixup = true;
      installPhase = "mkdir $out";
    });


    holochain-tests-nextest' = craneLib.cargoNextest (commonArgs // {
      pname = "holochain-nextest";
      __impure = pkgs.stdenv.isLinux;
      cargoArtifacts = holochainNextestDeps;
      preCheck = ''
        pwd
        # rm /build/source/target/debug/.fingerprint/holochain_wasm_test_utils-*/invoked.timestamp
        # rm /build/source/target/debug/.fingerprint/holochain_test_wasm_common-*/invoked.timestamp
      '';
      # cargoExtraArgs = "--features slow_tests,glacial_tests,test_utils,build_wasms,db-encryption";
      # CARGO_PROFILE = "release";
      cargoExtraArgs = ''
        ${import ../../.config/nextest-args.nix} \
        ${lib.concatStringsSep " " disabledTestsArgs}
      '';

      DYLD_PRINT_LIBRARIES=1;

      dontPatchELF = true;
      dontFixup = true;
      installPhase = "mkdir $out";

      # cargoNextestExtraArgs = lib.concatStringsSep " " disabledTestsArgs;
    });

    holochain-tests-nextest = holochain-tests-nextest'.overrideAttrs (old: {
      buildInputs = old.buildInputs
      ++ (lib.filter (b: lib.hasPrefix "rust-default-" b.name) old.nativeBuildInputs);
    });

  in {
    packages = {inherit holochain holochain-tests holochain-tests-nextest;};
  };
}
