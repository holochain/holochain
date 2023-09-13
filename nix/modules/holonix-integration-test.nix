{ self
, lib
, ...
} @ flake: {
  perSystem =
    { self'
    , config
    , pkgs
    , ...
    }:
    let
      bats = "${pkgs.bats}/bin/bats";
      testScript =
        pkgs.writeShellScript ""
          ''
            set -Eeuo pipefail
            cd ${flake.config.srcCleanedHolonix}/holonix

            ${bats} ./test/holochain-binaries.bats
            ${bats} ./test/launcher.bats
            ${bats} ./test/scaffolding.bats
            ${bats} ./test/rust.bats
            ${bats} ./test/hc-sandbox.bats
          '';
    in
    {
      packages.build-holonix-tests-integration = self'.devShells.holonix.overrideAttrs (old: {
        phases = [
          "buildPhase"
          "checkPhase"
        ];

        doCheck = true;

        nativeCheckInputs = [
          pkgs.coreutils
          pkgs.procps
        ];

        checkPhase = ''
          # output to console and to logfile
          exec >> >(tee $out) 2>&1

          echo =============== TESTSCRIPT OUTPUT STARTS HERE ===============
          ${testScript}
        '';

        preferLocalBuild = false;
      });

    };
}
