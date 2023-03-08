{ self, lib, ... }: {
  perSystem = { config, pkgs, ... }:
    {
      apps.holochain-nix-integration-test.type = "app";
      apps.holochain-nix-integration-test.program = builtins.toString
        (pkgs.writeShellScript "script.sh" ''
          set -xeu

          export PATH="${
            lib.makeBinPath (with pkgs; [ gitFull coreutils nix niv ])
          }:$PATH"

          # remove everything that wouldn't be on github either
          git clean -fdx

          # we use this git daemon to not rely on the published tag
          git daemon --reuseaddr --base-path=. --export-all --verbose --detach

          git clone "''${HOLOCHAIN_NIXPKGS_URL}" "''${HOLOCHAIN_NIXPKGS_REPO}" -b ''${HOLOCHAIN_NIXPKGS_SOURCE_BRANCH} --depth=1
          cd "''${HOLOCHAIN_NIXPKGS_REPO}"

          git checkout -b "''${RELEASE_BRANCH}"

          if grep --quiet ''${VERSION_COMPAT} packages/holochain/versions/update_config.toml; then
            export VERSION_COMPAT="''${VERSION_COMPAT}-ci"
            export TAG="''${TAG}-ci"
            git -C "''${HOLOCHAIN_REPO}" tag --force "''${TAG}"
          fi

          # TODO: use a util from the holochain-nixpkgs repo to make this change as this can get out of sync
          cat <<EOF >> packages/holochain/versions/update_config.toml

          [''${VERSION_COMPAT}]
          git-src = "revision:''${TAG}"
          git-repo = "git://localhost/"
          EOF

          # regenerate the nix sources
          git config --global user.email "devcore@holochain.org"
          git config --global user.name "Holochain Core Dev Team"
          nix-shell \
            --pure \
            --keep VERSION_COMPAT \
            --arg flavors '["release"]' \
            --run 'hnixpkgs-update-single ''${VERSION_COMPAT}'
          nix-build . -A packages.holochain.holochainAllBinariesWithDeps.''${VERSION_COMPAT} --no-link

          cd "''${HOLONIX_REPO}"

          niv drop holochain-nixpkgs
          niv add local --path "''${HOLOCHAIN_NIXPKGS_REPO}" --name holochain-nixpkgs

          # this should be the same as ''${TAG}
          nix eval -f ./ holochain-nixpkgs.packages.holochain.holochainAllBinariesWithDeps.''${VERSION_COMPAT}.holochain.src.rev

          # TODO: replace the following by 'nix run .#holonix-integration-test'
          nix-shell \
            --pure \
            --argstr holochainVersionId "''${VERSION_COMPAT}" \
            --arg include '{ test = true; }' \
            --run '
              holochain --version
              hn-test
            '
        '');
    };
}
