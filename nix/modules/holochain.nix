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
        src = flake.config.srcCleanedHolochain;

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
      holochain = craneLib.buildPackage (commonArgs // {
        CARGO_PROFILE = "release";
        cargoArtifacts = holochainDepsRelease;
        src = flake.config.srcCleanedHolochain;
        doCheck = false;
        passthru.src.rev = inputs.holochain.rev;
      });

      holochainNextestDeps = craneLib.buildDepsOnly (commonArgs // {
        pname = "holochain-tests-nextest";
        CARGO_PROFILE = "fast-test";
        nativeBuildInputs = [ pkgs.cargo-nextest ];
        buildPhase = ''
          cargo nextest run --no-run \
          ${import ../../.config/test-args.nix} \
          ${import ../../.config/nextest-args.nix} \
        '';
        dontCheck = true;
      });

      holochainTestsNextestArgs =
        let
          # e.g.
          # "conductor::cell::gossip_test::gossip_test"
          disabledTests = [
            "core::ribosome::host_fn::remote_signal::tests::remote_signal_test"
            "new_lair::test_new_lair_conductor_integration"
            "conductor::cell::gossip_test::gossip_test"
          ] ++ (lib.optionals (pkgs.system == "x86_64-darwin") [
            "test_reconnect"
            "timeout::tests::kitsune_backoff"
            "test_util::switchboard::tests::transitive_peer_gossip"
          ]) ++ (lib.optionals (pkgs.system == "aarch64-darwin") [
            "test_reconnect"
          ]);

          # the space after the not is crucial or else nextest won't parse the expression
          disabledTestsArg = '' \
            -E 'not test(/${lib.concatStringsSep "|" disabledTests}/)' \
          '';
        in
        (commonArgs // {
          __noChroot = pkgs.stdenv.isLinux;
          cargoArtifacts = holochainNextestDeps;

          pname = "holochain-tests-nextest";

          preCheck = ''
            export DYLD_FALLBACK_LIBRARY_PATH=$(rustc --print sysroot)/lib
          '';

          cargoExtraArgs = ''
            --profile ci \
            --config-file ${../../.config/nextest.toml} \
            ${import ../../.config/test-args.nix} \
            ${import ../../.config/nextest-args.nix} \
            ${disabledTestsArg} \
          '';

          dontPatchELF = true;
          dontFixup = true;

          installPhase = ''
            mkdir -p $out
            cp -vL target/.rustc_info.json $out/
            find target -name "junit.xml" -exec cp -vLb {} $out/ \;
          '';
        });

      build-holochain-tests-unit = craneLib.cargoNextest holochainTestsNextestArgs;
      build-holochain-tests-unit-tx5 = craneLib.cargoNextest
        (holochainTestsNextestArgs // {
          pname = "holochain-tests-nextest-tx5";
          cargoExtraArgs = holochainTestsNextestArgs.cargoExtraArgs + '' \
            --features tx5 \
          '';

          nativeBuildInputs = holochainTestsNextestArgs.nativeBuildInputs ++ [
            pkgs.go
          ];
        });

      build-holochain-tests-static-fmt = craneLib.cargoFmt (commonArgs // {
        src = flake.config.srcCleanedHolochain;
        cargoArtifacts = null;
        doCheck = false;

        dontPatchELF = true;
        dontFixup = true;
      });

      build-holochain-tests-static-clippy = craneLib.cargoClippy (commonArgs // {
        pname = "holochain-tests-clippy";
        src = flake.config.srcCleanedHolochain;
        cargoArtifacts = holochainDeps;
        doCheck = false;

        cargoClippyExtraArgs = "-- ${import ../../.config/clippy-args.nix}";

        dontPatchELF = true;
        dontFixup = true;

        installPhase = ''
          mkdir -p $out
          cp -vL target/.rustc_info.json  $out/
        '';
      });

      holochainWasmArgs = (commonArgs // {
        pname = "holochain-tests-wasm";

        postConfigure = ''
          export CARGO_TARGET_DIR=''${CARGO_TARGET_DIR:-$PWD/target}
        '';

        cargoExtraArgs =
          "--lib --all-features --manifest-path=crates/test_utils/wasm/wasm_workspace/Cargo.toml";

        cargoLock = "${flake.config.srcCleanedHolochain}/crates/test_utils/wasm/wasm_workspace/Cargo.lock";
      });

      holochainDepsWasm = craneLib.buildDepsOnly (holochainWasmArgs // {
        cargoArtifacts = null;
      });

      build-holochain-tests-unit-wasm = craneLib.cargoTest (holochainWasmArgs // {
        cargoArtifacts = holochainDepsWasm;

        dontPatchELF = true;
        dontFixup = true;

        installPhase = ''
          mkdir -p $out
          cp -vL target/.rustc_info.json  $out/
        '';
      });

      build-holochain-tests-static-doc = craneLib.cargoDoc (commonArgs // {
        pname = "holochain-tests-docs";
        cargoArtifacts = holochainDeps;
      });



      # meta packages to build multiple test packages at once
      build-holochain-tests-unit-all = config.lib.mkMetaPkg "holochain-tests-unit-all" [
        build-holochain-tests-unit
        build-holochain-tests-unit-tx5
        build-holochain-tests-unit-wasm
      ];

      build-holochain-tests-static-all = config.lib.mkMetaPkg "holochain-tests-static-all" [
        build-holochain-tests-static-doc
        build-holochain-tests-static-fmt
        build-holochain-tests-static-clippy
      ];

      build-holochain-tests-all = config.lib.mkMetaPkg "build-holochain-tests-all" [
        build-holochain-tests-unit-all
        build-holochain-tests-static-all
      ];

    in
    {
      packages =
        {
          inherit
            holochain

            build-holochain-tests-unit
            build-holochain-tests-unit-tx5
            build-holochain-tests-unit-wasm
            build-holochain-tests-unit-all

            build-holochain-tests-static-doc
            build-holochain-tests-static-fmt
            build-holochain-tests-static-clippy
            build-holochain-tests-static-all

            build-holochain-tests-all
            ;
        };
    };
}
