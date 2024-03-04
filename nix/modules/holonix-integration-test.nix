{ self
, lib
, inputs
, ...
} @ flake: {
  perSystem =
    { self'
    , config
    , pkgs
    , system
    , ...
    }:
    let

      rustToolchain = config.rustHelper.mkRust {
        track = "stable";
        version = "1.75.0";
      };
      craneLib = inputs.crane.lib.${system}.overrideToolchain rustToolchain;
      moldOpensslDeps = craneLib.vendorCargoDeps {
        src = "${flake.config.srcCleanedHolonix}/holonix/test/mold_openssl";
      };
    in
    {
      packages.build-holonix-tests-integration = pkgs.mkShell {
        inputsFrom = [ self'.devShells.holonix ];
        phases = [
          "buildPhase"
          "checkPhase"
        ];

        doCheck = true;

        nativeCheckInputs = [
          pkgs.coreutils
          pkgs.procps
          pkgs.killall
          pkgs.bats
        ];

        checkPhase = ''
          # output to console and to logfile
          exec >> >(tee $out) 2>&1

          eval "$shellHook"

          echo =============== TESTSCRIPT OUTPUT STARTS HERE ===============
          set -Eeuo pipefail

          cd ${flake.config.srcCleanedHolonix}/holonix

          bats ./test/shell-setup.bats
          bats ./test/holochain-binaries.bats
          bats ./test/launcher.bats
          bats ./test/scaffolding.bats
          bats ./test/rust.bats
          bats ./test/hc-sandbox.bats

          env CARGO_VENDOR_DIR="${moldOpensslDeps}" \
            bats ./test/mold_openssl.bats
        '' + lib.strings.optionalString pkgs.stdenv.isLinux ''
          bats ./test/shell-setup-linux.bats
        ''
        ;

        preferLocalBuild = false;
      };
    };
}
