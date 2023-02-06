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
      packages =
        {
          holonix-tests-integration = pkgs.runCommand
            "holonix-tests-integration"
            {
              __noChroot = pkgs.stdenv.isLinux;

              requiredSystemFeatures = [ "recursive-nix" ];

              nativeBuildInputs = [
                pkgs.nix
              ];
              buildInputs = [
                pkgs.cacert
              ];
            } ''

            export HOME=$PWD/.writeable_home
            mkdir -p $HOME

            set -x

            nix develop ${self}#holonix \
              --override-input versions/holochain ${self} \
              --extra-experimental-features "flakes nix-command" \
              --command ${testScript}

            echo this line causes ${self'.devShells.holonix} to be pre-built | tee $out

            set +x
          '';
        };
    };
}
