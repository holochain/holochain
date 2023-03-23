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
          '';
    in
    {
      packages.build-holonix-tests-integration = self'.devShells.holonix.overrideAttrs (old: {
        buildPhase = ''
          ${testScript} 2>&1 | ${pkgs.coreutils}/bin/tee $out
        '';
      });
    };
}
