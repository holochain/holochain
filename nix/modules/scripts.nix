{ self, lib, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }: {
    # Definitions like this are entirely equivalent to the ones
    # you may have directly in flake.nix.
    packages = {
      scripts-ci-cachix-helper =
        let
          pathPrefix = lib.makeBinPath (with pkgs; [ cachix ]);
        in
        pkgs.writeShellScript "scripts-ci-cachix-helper" ''
            #! /usr/bin/env nix-shell
            set -euo pipefail

          export PATH=${pathPrefix}:$PATH

          export PATHS_PREBUILD_FILE="''${HOME}/.store-path-pre-build"

          case ''${1} in
            setup)
              if [[ -n ''${CACHIX_AUTH_TOKEN:-} ]]; then
                  echo Using CACHIX_AUTH_TOKEN
                  cachix --verbose authtoken ''${CACHIX_AUTH_TOKEN}
              fi
              cachix --verbose use -m user-nixconf ''${CACHIX_NAME:?}
              nix path-info --all > "''${PATHS_PREBUILD_FILE}"
              ;;

            push)
              comm -13 <(sort "''${PATHS_PREBUILD_FILE}" | grep -v '\.drv$') <(nix path-info --all | grep -v '\.drv$' | sort) | cachix --verbose push ''${CACHIX_NAME:?}
              ;;
          esac
        '';

      scripts-repo-flake-update = pkgs.writeShellScriptBin "scripts-repo-flake-update" ''
        #! /usr/bin/env nix-shell
        set -xeuo pipefail
        trap "cd $PWD" EXIT

        cd versions/0_1
        nix flake update
        cd ../../

        nix flake lock --update-input versions --override-input versions "path:./versions/0_1"
        # nix flake lock --update-input versions
      '';
    };
  };
}
