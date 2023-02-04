# { self, inputs, lib, ... }@flake: {
#   perSystem = { config, self', inputs', system, pkgs, ... }: {
#     packages = {
#       release-automation =
#         pkgs.callPackage ../../crates/release-automation/default.nix {
#           crate2nixSrc = inputs.crate2nix;
#         };

#       release-automation-regenerate-readme =
#         pkgs.writeShellScriptBin "release-automation-regenerate-readme" ''
#           set -x
#           ${pkgs.cargo-readme}/bin/cargo-readme readme --project-root=crates/release-automation/ --output=README.md;
#         '';
#     };
#   };
# }

# Definitions can be imported from a separate file like this one

{ self, inputs, lib, ... }@flake: {
  perSystem = { config, self', inputs', system, pkgs, ... }:
    let

      rustToolchain = config.rust.mkRust {
        track = "stable";
        version = "latest";
      };
      craneLib = inputs.crane.lib.${system}.overrideToolchain rustToolchain;

      commonArgs = {
        pname = "release-automation";
        version = "workspace";
        src = flake.config.srcCleanedReleaseAutomationRepo;

        cargoExtraArgs = "--all-targets";

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
        pname = "${commonArgs.pname}-tests";
        __noChroot = pkgs.stdenv.isLinux;

        cargoArtifacts = deps;

        buildInputs = commonArgs.buildInputs ++ [ pkgs.cacert ];
        nativeBuildInputs = commonArgs.nativeBuildInputs ++
          [
            pkgs.gitFull
            (config.writers.writePureShellScriptBin
              "release-automation"
              ([ pkgs.gitFull rustToolchain ] ++ commonArgs.nativeBuildInputs ++ commonArgs.buildInputs)
              "exec ${package}/bin/release-automation $@")
          ];

        cargoNextestExtraArgs =
          let
            nextestToml = builtins.toFile "nextest.toml" ''
              [profile.default]
              retries = 1
              status-level = "skip"
              final-status-level = "flaky"
            '';
          in
          '' \
            --config-file=${nextestToml} \
          '';

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

        release-automation-tests = tests;

        # check the state of the repository
        # TODO: to get the actual .git repo we could be something like this:
        # using a dummy input like this:
        # ```nix
        #     repo-git.url = "file+file:/dev/null";
        #     repo-git.flake = false;
        # ```
        # and then when i run the test derivations that rely on that input, i can temporarily lock that input to a local path like this:
        # ```
        # tmpgit=$(mktemp -d)
        # git clone --bare --single-branch . $tmpgit
        # nix flake lock --update-input repo-git --override-input repo-git "path:$tmpdir"
        # rm -rf $tmpgit
        # ```
        release-automation-tests-repo = pkgs.runCommand
          "release-automation-tests-repo"
          {
            __noChroot = pkgs.stdenv.isLinux;
            nativeBuildInputs = self'.packages.holochainRepo.nativeBuildInputs ++ [
              package

              pkgs.coreutils
              pkgs.gitFull
            ];
            buildInputs = self'.packages.holochainRepo.buildInputs ++ [
              pkgs.cacert
            ];
          } ''
          set -euo pipefail

          export HOME="$(mktemp -d)"
          export TEST_WORKSPACE="''${HOME:?}/src"

          cp -r --no-preserve=mode,ownership ${flake.config.srcCleanedRepo} ''${TEST_WORKSPACE:?}
          cp --no-preserve=mode,ownership ${../../CHANGELOG.md} ''${TEST_WORKSPACE:?}/CHANGELOG.md
          cd ''${TEST_WORKSPACE:?}

          git init
          git switch -c main
          git add .
          git config --global user.email "you@example.com"
          git config --global user.name "Your Name"
          git commit -am "main"

          release-automation \
              --workspace-path=''${TEST_WORKSPACE:?} \
              --log-level=debug \
            crate \
              apply-dev-versions \
              --commit \
              --no-verify

          release-automation \
              --workspace-path=''${TEST_WORKSPACE:?} \
              --log-level=debug \
              --match-filter="^(holochain|holochain_cli|kitsune_p2p_proxy)$" \
            release \
              --no-verify-pre \
              --force-branch-creation \
              --disallowed-version-reqs=">=0.3" \
              --allowed-matched-blockers=UnreleasableViaChangelogFrontmatter \
              --steps=CreateReleaseBranch,BumpReleaseVersions

          rm -rf target
          mv ''${TEST_WORKSPACE:?} $out
        '';
      };
    };
}
