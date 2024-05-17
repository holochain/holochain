# Definitions can be imported from a separate file like this one

{ self, inputs, lib, ... }@flake: {
  perSystem = { config, self', inputs', system, pkgs, ... }:
    let
      rustToolchain = config.rustHelper.mkRust {
        track = "stable";
        version = "1.77.2";
      };

      craneLib = inputs.crane.lib.${system}.overrideToolchain rustToolchain;

      commonArgs = {
        RUST_SODIUM_LIB_DIR = "${pkgs.libsodium}/lib";
        RUST_SODIUM_SHARED = "1";

        pname = "holochain";
        src = flake.config.srcCleanedHolochain;

        version = "workspace";

        CARGO_PROFILE = "";

        buildInputs = (with pkgs; [ openssl self'.packages.opensslStatic sqlcipher ])
          ++ (lib.optionals pkgs.stdenv.isDarwin
          (with pkgs.darwin.apple_sdk_11_0.frameworks; [
            AppKit
            CoreFoundation
            CoreServices
            Security
            IOKit
          ]));

        nativeBuildInputs = (with pkgs; [ makeWrapper perl pkg-config self'.packages.goWrapper ])
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

      holochain_chc = holochain.override { cargoExtraArgs = " --features chc"; };

      holochainNextestDeps = craneLib.buildDepsOnly (commonArgs // {
        pname = "holochain-tests-nextest";
        CARGO_PROFILE = "fast-test";
        nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pkgs.cargo-nextest ];
        buildPhase = ''
          cargo nextest run --no-run \
          ${import ../../.config/test-args.nix} \
          ${import ../../.config/nextest-args.nix} \
        '';
        dontCheck = true;
      });

      holochainTestsNextestArgs =
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
            ${import ../../.config/nextest-args.nix}
          '';

          cargoNextestExtraArgs = builtins.getEnv "NEXTEST_EXTRA_ARGS";

          dontPatchELF = true;
          dontFixup = true;

          nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ holochain ];

          installPhase = ''
            mkdir -p $out
            cp -vL target/.rustc_info.json $out/
            find target -name "junit.xml" -exec cp -vLb {} $out/ \;
          '';
        });

      build-holochain-tests-unit = lib.makeOverridable craneLib.cargoNextest holochainTestsNextestArgs;

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

        cargoClippyExtraArgs =
          let
            # contains a set with items like 'nursery = allow'
            workspaceClippyLints = (builtins.fromTOML (builtins.readFile "${self}/Cargo.toml")).workspace.lints.clippy;
            workspaceClippyLints1 = builtins.mapAttrs
              (name: value:
                builtins.concatStringsSep " " [
                  (
                    if value == "allow" then "-A"
                    else if value == "deny" then "-D"
                    else throw "unsupported lint: ${name} = ${value}"
                  )
                  "clippy::${name}"
                ]
              )
              workspaceClippyLints
            ;

            # contains a list of e.g. "-A clippy::nursery"
            workspaceClippyLints2 = builtins.attrValues workspaceClippyLints1;

            # contains the final argument string
            workspaceClippyLints3 = builtins.concatStringsSep " " workspaceClippyLints2;
          in
          # the outcome will be: "-- -A clippy::nursery -D ..."
          "-- ${workspaceClippyLints3}"
        ;

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
            holochain_chc

            build-holochain-tests-unit
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
