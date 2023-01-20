{ self, lib, ... } @ flake: {
  perSystem = { config, self', inputs', pkgs, ... }: {
    apps.holonix-integration-test.type = "app";
    apps.holonix-integration-test.program = let
      script = config.writers.writePureShellScript
        [
          pkgs.coreutils
          pkgs.nix
        ]
        ''
          # cp -r ${flake.config.sources.holonix} $TMPDIR/holonix

          # cd $TMPDIR/holonix
          cd ${flake.config.sources.holonix}
          nix-shell --run "bash -c '
            holochain --version
          '"
        '';

      hn-test = config.writers.writePureShellScript
        [
          pkgs.bats
          pkgs.coreutils
          pkgs.nix
        ]
        ''
          cp -r ${flake.config.sources.holonix} $TMPDIR/holonix
          cd $TMPDIR/holonix

          nix-shell --run "bash -c '
            bats ./test/clippy.bats
            # TODO: revisit when decided on a new gihtub-release binary
            # bats ./test/github-release.bats

            bats ./test/nix-shell.bats
            ${if pkgs.stdenv.isLinux then "bats ./test/perf.bats" else ""}
            bats ./test/rust-manifest-list-unpinned.bats
            bats ./test/rust.bats
            bats ./test/flamegraph.bats
            bats ./test/holochain-binaries.bats

            # TODO:
            #   Decide what to do with these tests.
            #   Currently launcher and scaffolding are not part of the shell
            # bats ./test/launcher.bats
            # bats ./test/scaffolding.bats
          '"
        '';

    in
      toString hn-test;
  };
}
