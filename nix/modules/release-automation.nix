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
        src = ../../crates/release-automation;

        CARGO_PROFILE = "release";

        buildInputs = (with pkgs; [ openssl ])
          ++ (lib.optionals pkgs.stdenv.isDarwin
          (with pkgs.darwin.apple_sdk_11_0.frameworks; [
            AppKit
            CoreFoundation
            CoreServices
            Security
          ]));

        nativeBuildInputs = (with pkgs; [ perl pkg-config ])
          ++ lib.optionals pkgs.stdenv.isDarwin
          (with pkgs; [ xcbuild libiconv ]);
      };

      # derivation building all dependencies
      deps = craneLib.buildDepsOnly (commonArgs // { });

      # derivation with the main crates
      package = craneLib.buildPackage (commonArgs // {
        cargoArtifacts = deps;
        doCheck = false;
      });

      testDeps = craneLib.buildDepsOnly (commonArgs // rec {
        RUST_SODIUM_LIB_DIR = "${pkgs.libsodium}/lib";
        RUST_SODIUM_SHARED = "1";
        cargoExtraArgs = "--tests";
      });

      # TODO: this currently fails becasue of the test environment that nix provides, despite __impure
      tests = craneLib.cargoTest (commonArgs // {
        __impure = pkgs.stdenv.isLinux;

        cargoArtifacts = testDeps;

        buildInputs = commonArgs.buildInputs ++ [ pkgs.cacert ];
        nativeBuildInputs = commonArgs.nativeBuildInputs
          ++ [ package pkgs.gitFull ];

        dontPatchELF = true;
        dontFixup = true;
        installPhase = "mkdir $out";
      });

      packagePath = lib.makeBinPath [ package ];

    in
    {
      packages = {
        release-automation = package;

        release-automation-tests-drv = tests;

        release-automation-tests =
          pkgs.writeShellScriptBin "release-automation-tests" ''
            set -euxo pipefail
            export RUST_BACKTRACE=1

            export PATH=${lib.makeBinPath (with pkgs; [ package nix gitFull ])}:$PATH

            nix-shell --argstr devShellId release --run '
              # make sure the binary is built
              cargo build --locked --manifest-path=crates/release-automation/Cargo.toml
              # run the release-automation tests
              cargo test ''${CARGO_TEST_ARGS:-} --locked --manifest-path=crates/release-automation/Cargo.toml ''${@}
            '
          '';

        # check the state of the repository
        release-automation-tests-repo = pkgs.writeShellScriptBin "release-automation-tests-repo" ''
          set -euxo pipefail

          export PATH=${lib.makeBinPath (with pkgs; [ package nix gitFull ])}:$PATH

          export TEST_WORKSPACE=$(mktemp -d)
          if [[ "''${KEEP_TEST_WORKSPACE:-false}" != "true" ]]; then
            trap "rm -rf ''${TEST_WORKSPACE:?}" EXIT
          fi

          nix-shell --argstr devShellId coreDev --run '
            rm -rf ''${TEST_WORKSPACE:?}
            git clone $PWD ''${TEST_WORKSPACE:?}
            cd ''${TEST_WORKSPACE:?}
            git switch -c release-automation-tests-repo

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
          '
        '';
      };
    };
}
