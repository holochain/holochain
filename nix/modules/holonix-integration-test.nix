{ self, lib, ... }: {
  perSystem = { self', config, pkgs, ... }:

    let
      bats = "${pkgs.bats}/bin/bats";
      testScript = pkgs.writeShellScript ""
        ''
          set - Eeuo pipefail

          ${bats} ./test/holochain-binaries.bats
          ${bats} ./test/launcher.bats
          ${bats} ./test/scaffolding.bats
          ${bats} ./test/rust.bats
        '';

    in
    {
      packages =
        {
          holonix-tests-integration = pkgs.runCommand
            "holonix-tests-integration"
            {
              # __noChroot = pkgs.stdenv.isLinux;
              __impure = pkgs.stdenv.isLinux;

              requiredSystemFeatures = [ "recursive-nix" ];

              nativeBuildInputs = [
                pkgs.nix
              ];
              buildInputs = [
                pkgs.cacert
              ];
            } ''

            mkdir .writable_home
            export HOME=$PWD/.writeable_home

            nix develop ${self'.devShells.holonix}
              --override-inputs holochain ${self} \
              --extra-experimental-features "flakes nix-command" \
              --command ${testScript}
          '';
        };
    };
}
