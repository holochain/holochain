# Definitions can be imported from a separate file like this one

{ self, inputs, lib, ... }@flake: {
  perSystem = { config, self', inputs', system, pkgs, ... }:
    let
      rustToolchain = config.rust.mkRust {
        track = "stable";
        version = "1.66.1";
      };

      craneLib = inputs.crane.lib.${system}.overrideToolchain rustToolchain;

      opensslStatic = pkgs.pkgsStatic.openssl;

      commonArgs = {
        RUST_SODIUM_LIB_DIR = "${pkgs.libsodium}/lib";
        RUST_SODIUM_SHARED = "1";

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
            IOKit
          ]));

        nativeBuildInputs = (with pkgs; [ makeWrapper perl pkg-config ])
          ++ lib.optionals pkgs.stdenv.isDarwin
          (with pkgs; [ xcbuild libiconv ]);
      };

      # derivation building all dependencies
      holochainDeps = craneLib.buildDepsOnly (commonArgs // rec {
        doCheck = false;
      });

      # derivation with the main crates
      holochain = craneLib.buildPackage (commonArgs // {
        cargoExtraArgs = '' \
          --bin hc-sandbox \
          --bin hc-app \
          --bin hc-dna \
          --bin hc \
          --bin hc-web-app \
          --bin holochain \
        '';
        cargoArtifacts = holochainDeps;
        doCheck = false;
      });

      holochainNextestDeps = craneLib.buildDepsOnly (commonArgs // rec {
        pname = "holochain-nextest";
        CARGO_PROFILE = "fast-test";
        cargoExtraArgs =
          "--features slow_tests,glacial_tests,test_utils,build_wasms,db-encryption --lib --tests";
        nativeBuildInputs = [ pkgs.cargo-nextest ];
        buildPhase = ''
          cargo nextest run --no-run \
          ${import ../../.config/test-args.nix} \
          ${import ../../.config/nextest-args.nix} \
        '';
        dontCheck = true;
      });

      # e.g.
      # "conductor::cell::gossip_test::gossip_test"
      disabledTests = [
        "core::ribosome::host_fn::remote_signal::tests::remote_signal_test"
      ];

      disabledTestsArgs =
        lib.forEach disabledTests (test: "-E 'not test(${test})'");

      holochain-tests-nextest = craneLib.cargoNextest (commonArgs // {
        pname = "holochain";
        __impure = pkgs.stdenv.isLinux;
        cargoArtifacts = holochainNextestDeps;


        nativeBuildInputs = commonArgs.nativeBuildInputs ++ (with pkgs; [
          gitFull
        ]);

        preCheck = ''
          export DYLD_FALLBACK_LIBRARY_PATH=$(rustc --print sysroot)/lib
        '';

        cargoExtraArgs = ''
          --config-file ${../../.config/nextest.toml} \
          ${import ../../.config/test-args.nix} \
          ${import ../../.config/nextest-args.nix} \
          ${lib.concatStringsSep " " disabledTestsArgs}
        '';

        dontPatchELF = true;
        dontFixup = true;
        dontInstall = true;
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
        inherit holochain holochain-tests-nextest;

        inherit holochain-tests-wasm holochain-tests-fmt holochain-tests-clippy;
      };
    };
}
