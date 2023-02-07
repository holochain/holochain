{ self, lib, ... }: {
  perSystem = { self', config, pkgs, ... }:

    let
      bats = "${pkgs.bats}/bin/bats";
      testScript = pkgs.writeShellScript ""
        ''
          set -Eeuo pipefail
          cd ${self}/holonix

          ${bats} ./test/holochain-binaries.bats
          ${bats} ./test/launcher.bats
          ${bats} ./test/scaffolding.bats
          ${bats} ./test/rust.bats
        '';

    in
    {
      packages.holonix-tests-integration =
        self'.devShells.holonix.overrideAttrs (old: {
          buildPhase = ''
            ${testScript}
            touch $out
          '';
        });
      };
}
