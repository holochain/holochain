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
      holochainDeps = craneLib.buildDepsOnly (commonArgs // rec {
        src = flake.config.srcCleanedHolochain;
        doCheck = false;
      });

      # derivation with the main crates
      holochain = craneLib.buildPackage (commonArgs // {
        cargoArtifacts = holochainDeps;
        src = flake.config.srcCleanedHolochain;
        doCheck = false;
      });

      holochainNextestDeps = craneLib.buildDepsOnly (commonArgs // rec {
        pname = "holochain-nextest";
        CARGO_PROFILE = "fast-test";
        nativeBuildInputs = [ pkgs.cargo-nextest ];
        buildPhase = ''
          cargo nextest run --no-run \
          ${import ../../.config/test-args.nix} \
          ${import ../../.config/nextest-args.nix} \
        '';
        dontCheck = true;
      });

      holochain-tests-nextest =
        let
          # e.g.
          # "conductor::cell::gossip_test::gossip_test"
          disabledTests = [
            "core::ribosome::host_fn::remote_signal::tests::remote_signal_test"
            "new_lair::test_new_lair_conductor_integration"
            "conductor::cell::gossip_test::gossip_test"
          ] ++ (lib.optionals (pkgs.system == "x86_64-darwin") [
          ]);

          # the space after the not is crucial or else nextest won't parse the expression
          disabledTestsArg = '' \
            -E 'not test(/${lib.concatStringsSep "|" disabledTests}/)'
          '';
        in
        craneLib.cargoNextest (commonArgs // {
          __noChroot = pkgs.stdenv.isLinux;
          cargoArtifacts = holochainNextestDeps;

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

        installPhase = ''
          mkdir -p $out
          cp -vL target/.rustc_info.json  $out/
        '';
      });

      holochain-tests-wasm = craneLib.cargoTest (commonArgs // {
        cargoArtifacts = holochainDeps;
        cargoExtraArgs =
          "--lib --manifest-path=crates/test_utils/wasm/wasm_workspace/Cargo.toml --all-features";

        dontPatchELF = true;
        dontFixup = true;

        installPhase = ''
          mkdir -p $out
          cp -vL target/.rustc_info.json  $out/
        '';
      });

      holochain-tests-doc = craneLib.cargoDoc (commonArgs // {
        cargoArtifacts = holochainDeps;
      });

    in
    {
      packages = {
        inherit holochain holochain-tests-nextest holochain-tests-nextest-tx5 holochain-tests-doc;

        inherit holochain-tests-wasm holochain-tests-fmt holochain-tests-clippy;
      };
    };
}
