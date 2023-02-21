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
        passthru.rev = inputs.holochain.rev;
      });

      # Tests if all workspace crates can be built via their own Cargo.toml,
      #   ignoring the workspace Cargo.toml file
      # Due to differences in the Cargo.toml's specified features, a package
      #   build can succees with the workspace's Cargo.toml but fail without it.
      # This ensures that individual packages can be installed from crates.io.
      holochain-crates-standalone = craneLib.buildPackage (commonArgs // rec {
        name = "holochain-crates-standalone";
        src = flake.config.srcCleanedHolochain;
        cargoArtifacts = holochainDepsRelease;
        CARGO_PROFILE = "release";
        doCheck = false;
        dontInstall = true;
        buildPhase = ''
          export workspace=$(realpath .)
          ${lib.concatStringsSep "\n" (map buildCommandCrate members)}
          touch $out
        '';
      });
      cargoToml = builtins.fromTOML (builtins.readFile (self + /Cargo.toml));
      members = cargoToml.workspace.members;
      buildCommandCrate = member: ''
        echo "changing to directory $workspace/${member} to build crate ${member}"
        cd $workspace/${member}
        cargo check \
          --profile $CARGO_PROFILE \
          --manifest-path=$workspace/${member}/Cargo.toml
      '';

      holochainNextestDeps = craneLib.buildDepsOnly (commonArgs // {
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

      holochain-tests-nextest = craneLib.cargoNextest holochainTestsNextestArgs;
      holochain-tests-nextest-tx5 = craneLib.cargoNextest
        (holochainTestsNextestArgs // {
          pname = "holochain-nextest-tx5";
          cargoExtraArgs = holochainTestsNextestArgs.cargoExtraArgs + '' \
            --features tx5 \
          '';

          nativeBuildInputs = holochainTestsNextestArgs.nativeBuildInputs ++ [
            pkgs.go
          ];
        });

      holochain-tests-fmt = craneLib.cargoFmt (commonArgs // {
        src = flake.config.srcCleanedHolochain;
        cargoArtifacts = null;
        doCheck = false;

        dontPatchELF = true;
        dontFixup = true;
      });

      holochain-tests-clippy = craneLib.cargoClippy (commonArgs // {
        pname = "holochain-tests-clippy";
        src = flake.config.srcCleanedHolochain;
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

      holochainWasmArgs = (commonArgs // {
        pname = "holochain-tests-wasm";
        cargoExtraArgs =
          "--lib --all-features";

        cargoToml = "${flake.config.srcCleanedHolochain}/crates/test_utils/wasm/wasm_workspace/Cargo.toml";
        cargoLock = "${flake.config.srcCleanedHolochain}/crates/test_utils/wasm/wasm_workspace/Cargo.lock";

        postUnpack = ''
          cd $sourceRoot/crates/test_utils/wasm/wasm_workspace
          sourceRoot="."
        '';
      });

      holochainDepsWasm = craneLib.buildDepsOnly (holochainWasmArgs // {
        cargoArtifacts = null;
      });

      holochain-tests-wasm = craneLib.cargoTest (holochainWasmArgs // {
        cargoArtifacts = holochainDepsWasm;

        dontPatchELF = true;
        dontFixup = true;

        installPhase = ''
          mkdir -p $out
          cp -vL target/.rustc_info.json  $out/
        '';
      });

      holochain-tests-doc = craneLib.cargoDoc (commonArgs // {
        pname = "holochain-tests-docs";
        cargoArtifacts = holochainDeps;
      });

    in
    {
      packages = {
        inherit
          holochain
          holochain-crates-standalone
          holochain-tests-nextest
          holochain-tests-nextest-tx5
          holochain-tests-doc
          holochain-tests-wasm
          holochain-tests-fmt
          holochain-tests-clippy
          ;
      };
    };
}
