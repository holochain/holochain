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

        export VERSIONS_DIR="./versions/''${1}"
        export DEFAULT_VERSIONS_DIR="$(nix flake metadata --no-write-lock-file --json | jq --raw-output '.locks.nodes.versions.locked.path')"

        (
          cd "$VERSIONS_DIR"
          nix flake update --tarball-ttl 0
        )

        if [[ $(git diff -- "$VERSIONS_DIR"/flake.lock | grep -E '^[+-]\s+"' | grep -v lastModified --count) -eq 0 ]]; then
          echo got no actual source changes, reverting modifications..
          git checkout $VERSIONS_DIR/flake.lock
          exit 0
        else
          git add "$VERSIONS_DIR"/flake.lock
        fi

        if [[ "$VERSIONS_DIR" == "$DEFAULT_VERSIONS_DIR" ]]; then
          nix flake lock --tarball-ttl 0 --update-input versions --override-input versions "path:$VERSIONS_DIR"
        fi

        if [[ $(git diff -- flake.lock | grep -E '^[+-]\s+"' | grep -v lastModified --count) -eq 0 ]]; then
          echo got no actual source changes in the toplevel flake.lock, reverting modifications..
          git checkout flake.lock
        else
          git add flake.lock
        fi

        git commit -m "chore(flakes): update $VERSIONS_DIR"
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
            --allowed-semver-increment-modes="!patch" \
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
