# Definitions can be imported from a separate file like this one

{ self, inputs, lib, ... }@flake: {
  perSystem = { config, self', inputs', system, pkgs, ... }:
    let
      rustToolchain = config.rustHelper.mkRust {
        track = "stable";
        version = "1.77.2";
      };

      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;

      commonArgs = {
        pname = "release-automation";
        version = "workspace";
        src = flake.config.srcCleanedReleaseAutomationRepo;

        buildInputs = (with pkgs; [ openssl ])
          ++ (lib.optionals pkgs.stdenv.isDarwin
          (with pkgs.darwin.apple_sdk_11_0.frameworks; [
            AppKit
            CoreFoundation
            CoreServices
            Security
          ]));

        nativeBuildInputs = (with pkgs;
          [ perl pkg-config ])
        ++ lib.optionals pkgs.stdenv.isDarwin
          (with pkgs; [ xcbuild libiconv ]);
      };

      # derivation building all dependencies
      deps = craneLib.buildDepsOnly (commonArgs // {
        doCheck = false;
      });

      # derivation with the main crates
      package = craneLib.buildPackage (commonArgs // {
        cargoArtifacts = deps;
        doCheck = false;
      });

      tests = craneLib.cargoNextest (commonArgs // {
        src = flake.config.srcCleanedReleaseAutomationWithTestsRepo;
        pname = "${commonArgs.pname}-tests";
        __noChroot = pkgs.stdenv.isLinux;

        cargoArtifacts = deps;

        buildInputs = commonArgs.buildInputs ++ [ pkgs.cacert ];
        nativeBuildInputs = commonArgs.nativeBuildInputs ++
          [
            package

            rustToolchain
            pkgs.gitFull
            pkgs.coreutils
          ];

        cargoNextestExtraArgs =
          let
            nextestToml = builtins.toFile "nextest.toml" ''
              [profile.default]
              retries = { backoff = "exponential", count = 3, delay = "1s", jitter = true }
              status-level = "skip"
              final-status-level = "flaky"

            '';
          in
          '' \
            --config-file=${nextestToml} \
          '' + builtins.getEnv "NEXTEST_EXTRA_ARGS";

        dontPatchELF = true;
        dontFixup = true;
        installPhase = ''
          mkdir -p $out
          cp -vL target/.rustc_info.json $out/
        '';
      });

      packagePath = lib.makeBinPath [ package ];

    in
    {
      packages = {
        release-automation = package;

        build-release-automation-tests-unit = tests;

        # check the state of the repository
        # this is using a dummy input like this:
        # ```nix
        #     repo-git.url = "file+file:/dev/null";
        #     repo-git.flake = false;
        # ```
        # and then the test derivation is built it relies on that input being the local repo path. see the "holochain-build-and-test.yml" workflow.
        build-release-automation-tests-repo =
          let
            release-script = self'.packages.scripts-release-automation-check-and-bump;
            readmes-script = self'.packages.scripts-ci-generate-readmes;
          in
          pkgs.runCommand
            "release-automation-tests-repo"
            {
              __noChroot = pkgs.stdenv.isLinux;
              nativeBuildInputs = self'.packages.holochain.nativeBuildInputs ++ [
                pkgs.coreutils
                pkgs.gitFull
              ];
              buildInputs = self'.packages.holochain.buildInputs ++ [
                pkgs.cacert
              ];
            } ''
            set -xeuo pipefail

            export HOME="$(mktemp -d)"
            export TEST_WORKSPACE="''${HOME:?}/src"

            git config --global --add safe.directory ${inputs.repo-git}
            git clone --single-branch ${inputs.repo-git} ''${TEST_WORKSPACE}

            cd ''${TEST_WORKSPACE:?}
            ${../../scripts/ci-git-config.sh}
            git status
            git switch -c repo-test

            ${readmes-script}/bin/${readmes-script.name}
            ${release-script}/bin/${release-script.name} ''${TEST_WORKSPACE}

            set +e
            git clean -ffdx
            mv ''${TEST_WORKSPACE} $out
            echo use "nix-store --realise $out" to retrieve the result.
          '';
      };
    };
}
