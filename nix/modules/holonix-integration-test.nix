{ self, lib, ... }: {
  perSystem = { config, pkgs, ... }: {

    packages = {
      holonix-integration-test = let bats = "${pkgs.bats}/bin/bats";
      in pkgs.writeShellScriptBin "holonix-integration-test" ''
        set -Eeuo pipefail

        cd holonix
        ${pkgs.nix}/bin/nix-shell --pure --run '
          set -Eeuo pipefail

          ${bats} ./test/holochain-binaries.bats
          ${bats} ./test/launcher.bats
          ${bats} ./test/scaffolding.bats
          ${bats} ./test/rust.bats
        '
      '';
    };
  };
}
