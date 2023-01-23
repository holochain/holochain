{ self, lib, ... }: {
  perSystem = { config, ... }: let
    pkgs = config.pkgs;
  in {
    apps.holonix-integration-test.type = "app";
    apps.holonix-integration-test.program = builtins.toString (
      config.writers.writePureShellScript
      [
        pkgs.bats
        pkgs.coreutils
        pkgs.nix
      ]
      ''
        # cp -r ${self} $TMPDIR/holonix
        # cd $TMPDIR/holonix

        cd ${self}/holonix
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
          bats ./test/launcher.bats
          bats ./test/scaffolding.bats
        '"
      ''
    );
  };
}
