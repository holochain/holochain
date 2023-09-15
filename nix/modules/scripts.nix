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
        set -xeuo pipefail
        trap "cd $PWD" EXIT

        export VERSIONS_DIR="versions/''${1}"
        export DEFAULT_VERSIONS_DIR="$(nix flake metadata --no-write-lock-file --json | jq --raw-output '.locks.nodes.versions.original.dir')"

        (
          cd "''${VERSIONS_DIR}"
          nix flake update
          jq . < flake.lock | grep -v revCount | grep -v lastModified > flake.lock.new
          mv flake.lock{.new,}
        )

        if [[ $(${pkgs.git}/bin/git diff -- ''${VERSIONS_DIR}/flake.lock | grep -E '^[+-]\s+"' --count) -eq 0 ]]; then
          echo got no actual source changes, reverting modifications..;
          ${pkgs.git}/bin/git checkout ''${VERSIONS_DIR}/flake.lock
          exit 0
        else
          git commit -m "chore(flakes) [1/2]: update ''${VERSIONS_DIR}" ''${VERSIONS_DIR}/flake.lock
        fi

        # "locked": {
        #   "lastModified": 1694809450,
        #   "narHash": "sha256-+iMesjheOJaz2cgynF6WBR2rCEX8iSPxPq15+9JVGyo=",
        #   "path": "versions/0_1",
        #   "type": "path"
        # },

        # "locked": {
        #   "dir": "versions/0_1",
        #   "lastModified": 1694803748,
        #   "narHash": "sha256-flpSTyaLCXm0LJenk2pxh8RjAsih0gpWvOK4pSk6nck=",
        #   "owner": "holochain",
        #   "repo": "holochain",
        #   "rev": "35ffa0134a126c7d028e420686aae33d220939a7",
        #   "type": "github"
        # },


        # "locked": {
        #   "dir": "versions/0_1",
        #   "lastModified": 1694807849,
        #   "narHash": "sha256-s2FzrqaCpuIg8Mw+QcFe8L/QtWAB6p5vywzLpAfFot8=",
        #   "type": "git",
        #   "url": "file:///home/steveej/src/holo/holochain?dir=versions%2f0_1"
        # },

        # "dir": "versions/0_1",
        # "lastModified": 1694803748,
        # "narHash": "sha256-flpSTyaLCXm0LJenk2pxh8RjAsih0gpWvOK4pSk6nck=",
        # "ref": "refs/heads/pr_flake_lock_mangling",
        # "rev": "35ffa0134a126c7d028e420686aae33d220939a7",
        # "revCount": 11505,
        # "type": "git",
        # "url": "file:///home/steveej/src/holo/holochain?dir=versions%2f0_1"


        if [[ "$VERSIONS_DIR" == "$DEFAULT_VERSIONS_DIR" ]]; then
          rev=$(git show-ref --hash HEAD)

          nix flake lock --tarball-ttl 0 --update-input versions --override-input versions "./''${VERSIONS_DIR}?rev=$rev"
          # replace the URL of the versions flake with the github URL
          nix eval --impure --json --expr "
            let
              lib = (import ${pkgs.path} {}).lib;
              removeByPath = pathList: set:
                lib.updateManyAttrsByPath [
                  {
                    path = lib.init pathList;
                    update = old:
                      lib.filterAttrs (n: v: n != (lib.last pathList)) old;
                  }
                ] set;
              lock = builtins.fromJSON (builtins.readFile ./flake.lock);
              lock_updated = removeByPath [ \"nodes\" \"versions\" \"locked\" \"revCount\" ] (lib.recursiveUpdate lock {
                nodes.versions.locked = {
                  inherit (lock.nodes.versions.locked)
                    lastModified
                    narHash
                    ;

                  # type = \"github\";
                  # owner =  \"holochain\";
                  # repo= \"holochain\";
                  rev = \"$rev\";
                  ref = \"develop\";
                  dir = \"''${VERSIONS_DIR}\";
                  url = \"github:holochain/holochain?dir=''${VERSIONS_DIR}\";
                };
              });
            in lock_updated
          " | ${pkgs.jq}/bin/jq --raw-output . > flake.lock.new
          mv flake.lock{.new,}
        fi

        if [[ $(git diff -- flake.lock | grep -E '^[+-]\s+"' | grep -v lastModified --count) -eq 0 ]]; then
          echo got no actual source changes in the toplevel flake.lock, reverting modifications..
          git checkout flake.lock
          exit 0
        fi

        echo git commit -m "chore(flakes) [2/2]: update ''${VERSIONS_DIR}" flake.lock
      '';

      scripts-release-automation-check-and-bump = pkgs.writeShellScriptBin "scripts-release-automation-check-and-bump" ''
        set -xeuo pipefail

        export WORKSPACE_PATH=''${1}

        ${self'.packages.release-automation}/bin/release-automation \
            --workspace-path=''${WORKSPACE_PATH} \
            --log-level=debug \
            crate detect-missing-releaseheadings

        ${self'.packages.release-automation}/bin/release-automation \
          --workspace-path=''${WORKSPACE_PATH} \
          --log-level=debug \
          --match-filter="^(holochain|holochain_cli|kitsune_p2p_proxy)$" \
          release \
            --no-verify \
            --force-tag-creation \
            --force-branch-creation \
            --additional-manifests="crates/test_utils/wasm/wasm_workspace/Cargo.toml" \
            --allowed-semver-increment-modes="!pre_minor beta-dev" \
            --steps=CreateReleaseBranch,BumpReleaseVersions

        ${self'.packages.release-automation}/bin/release-automation \
            --workspace-path=''${WORKSPACE_PATH} \
            --log-level=debug \
            release \
              --dry-run \
              --no-verify \
              --steps=PublishToCratesIo
      '';

      scripts-ci-generate-readmes =
        let
          pathPrefix = lib.makeBinPath [
            self'.packages.cargo-rdme
            pkgs.cargo
            pkgs.rustc
            pkgs.gitFull
          ];

          crates = [
            "hdi"
            "hdk"
            "holochain_keystore"
            "holochain_state"
          ];
        in
        pkgs.writeShellScriptBin "scripts-ci-generate-readmes" ''
          set -xeuo pipefail

          export PATH=${pathPrefix}:$PATH

          crates_to_document=(${builtins.concatStringsSep " " crates})

          for crate in "''${crates_to_document[@]}"; do
            echo 'generating README for crate' "$crate"
            cargo-rdme -w $crate --intralinks-strip-links --force
          done

          # have any READMEs been updated?
          changed_readmes=$(git diff --exit-code --name-only '**README.md' || :)
          if [[ -n "$changed_readmes" ]]; then
            echo 'READMEs have been updated, committing changes'
            ${../../scripts/ci-git-config.sh}
            git commit -m "docs(crate-level): generate readmes from doc comments" $changed_readmes
          fi
        '';
    };

  };
}
