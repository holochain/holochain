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

        if [[ $(${pkgs.git}/bin/git diff -- "$VERSIONS_DIR"/flake.lock | grep -E '^[+-]\s+"' | grep -v lastModified --count) -eq 0 ]]; then
          echo got no actual source changes, reverting modifications..
          ${pkgs.git}/bin/git checkout $VERSIONS_DIR/flake.lock
          exit 0
        else
          git add "$VERSIONS_DIR"/flake.lock
        fi

        if [[ "$VERSIONS_DIR" = "$DEFAULT_VERSIONS_DIR" ]]; then
          nix flake lock --tarball-ttl 0 --update-input versions --override-input versions "path:$VERSIONS_DIR" 
        fi

        if [[ $(${pkgs.git}/bin/git diff -- flake.lock | grep -E '^[+-]\s+"' | grep -v lastModified --count) -eq 0 ]]; then
          echo got no actual source changes in the toplevel flake.lock, reverting modifications..
          ${pkgs.git}/bin/git checkout flake.lock
        else
          git add flake.lock
        fi

        git commit -m "chore(flakes): update $VERSIONS_DIR"
      '';

      scripts-release-automation-check-and-bump = pkgs.writeShellScriptBin "scripts-release-automation-check-and-bump" ''
        set -xeuo pipefail

        ${self'.packages.release-automation}/bin/release-automation \
            --workspace-path=$PWD \
            --log-level=debug \
            crate detect-missing-releaseheadings

        ${self'.packages.release-automation}/bin/release-automation \
          --workspace-path=''${1} \
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
            --workspace-path=''${1} \
            --log-level=debug \
            release \
              --dry-run \
              --no-verify \
              --steps=PublishToCratesIo
      '';

      scripts-ci-generate-readmes = pkgs.writeShellScriptBin "scripts-ci-generate-readmes" ''
        crates_to_document=("hdi" "hdk" "holochain_keystore" "holochain_state")

        for crate in "''${crates_to_document[@]}"; do
            echo 'generating README for crate' "$crate"
            ${self'.packages.cargo-rdme} -w $crate --intralinks-strip-links --force
        done

        # have any READMEs been updated?
        git diff --exit-code --quiet
        readmes_updated=$?
        if [[ "$readmes_updated" == 1 ]]; then
            echo 'READMEs have been updated, committing changes'
            git config --local user.name release-ci
            git config --local user.email ci@holo.host
            git commit -am "docs(crate-level): generate readmes from doc comments"
            git config --local --unset user.name
            git config --local --unset user.email
        fi
      '';
    };

  };
}
