# Definitions can be imported from a separate file like this one

{ self, inputs, lib, ... }@flake: {
  perSystem = { config, self', inputs', system, ... }:
    let

      pkgs = config.pkgs;

      rustToolchain = config.rust.rustHolochain;
      craneLib = inputs.crane.lib.${system}.overrideToolchain rustToolchain;

      opensslStatic = pkgs.pkgsStatic.openssl;

      commonArgs = {

        pname = "holochain";
        src = flake.config.srcCleaned;

        version = "workspace";

        CARGO_PROFILE = "";

        OPENSSL_NO_VENDOR = "1";
        OPENSSL_LIB_DIR = "${opensslStatic.out}/lib";
        OPENSSL_INCLUDE_DIR = "${opensslStatic.dev}/include";

        buildInputs = (with pkgs; [ openssl opensslStatic sqlcipher ])
          ++ (lib.optionals pkgs.stdenv.isDarwin
          (with pkgs.darwin.apple_sdk_11_0.frameworks; [
            AppKit
            CoreFoundation
            CoreServices
            Security
          ]));

        nativeBuildInputs = (with pkgs; [ makeWrapper perl pkg-config ])
          ++ lib.optionals pkgs.stdenv.isDarwin
          (with pkgs; [ xcbuild libiconv ]);
      };

      # derivation building all dependencies
      holochainDeps = craneLib.buildDepsOnly (commonArgs // rec {
        RUST_SODIUM_LIB_DIR = "${pkgs.libsodium}/lib";
        RUST_SODIUM_SHARED = "1";
        doCheck = false;
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
        cargoExtraArgs =
          "--features slow_tests,glacial_tests,test_utils,build_wasms,db-encryption --lib --tests";
      });

      holochainNextestDeps = craneLib.buildDepsOnly (commonArgs // rec {
        RUST_SODIUM_LIB_DIR = "${pkgs.libsodium}/lib";
        RUST_SODIUM_SHARED = "1";
        pname = "holochain-nextest";
        CARGO_PROFILE = "fast-test";
        cargoExtraArgs =
          "--features slow_tests,glacial_tests,test_utils,build_wasms,db-encryption --lib --tests";
        nativeBuildInputs = [ pkgs.cargo-nextest ];
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
        cargoExtraArgs =
          "--features slow_tests,glacial_tests,test_utils,build_wasms,db-encryption --lib --tests";

        dontPatchELF = true;
        dontFixup = true;
        installPhase = "mkdir $out";
      });

      holochain-tests-nextest = craneLib.cargoNextest (commonArgs // {
        pname = "holochain";
        __impure = pkgs.stdenv.isLinux;
        cargoArtifacts = holochainNextestDeps;

        # This was needed if __impure is not set. But since the tests were slow,
        #   we disabled the sandbox anyways. This might be needed in the future.
        # preCheck = ''
        #   rm /build/source/target/debug/.fingerprint/holochain_wasm_test_utils-*/invoked.timestamp
        #   rm /build/source/target/debug/.fingerprint/holochain_test_wasm_common-*/invoked.timestamp
        # '';

        preCheck = ''
          export DYLD_FALLBACK_LIBRARY_PATH=$(rustc --print sysroot)/lib
        '';

        cargoExtraArgs = ''
          ${import ../../.config/nextest-args.nix} \
          ${lib.concatStringsSep " " disabledTestsArgs}
        '';

        dontPatchELF = true;
        dontFixup = true;
        dontInstall = true;

        # TODO: fix upstream bug that seems to ignore `cargoNextestExtraArgs`
        # cargoNextestExtraArgs = lib.concatStringsSep " " disabledTestsArgs;
      });

      holochain-tests-fmt = craneLib.cargoFmt (commonArgs // {
        cargoArtifacts = null;
        doCheck = false;

        dontPatchELF = true;
        dontFixup = true;
      });

      holochain-tests-clippy = craneLib.cargoClippy (commonArgs // {
        cargoArtifacts = holochainDeps;
        doCheck = false;

        cargoClippyExtraArgs = ''
          -- \
          -A clippy::nursery -D clippy::style -A clippy::cargo \
          -A clippy::pedantic -A clippy::restriction \
          -D clippy::complexity -D clippy::perf -D clippy::correctness
        '';

        dontPatchELF = true;
        dontFixup = true;
      });

      holochain-tests-wasm = craneLib.cargoTest (commonArgs // {
        cargoArtifacts = holochainDeps;
        cargoExtraArgs =
          "--lib --manifest-path=crates/test_utils/wasm/wasm_workspace/Cargo.toml --all-features";

        dontPatchELF = true;
        dontFixup = true;
      });

    in
    {
      packages = {
        inherit holochain holochain-tests holochain-tests-nextest;

        inherit holochain-tests-wasm holochain-tests-fmt holochain-tests-clippy;
      };
    };
}
