{ config
, isIncludedFn
, holochainVersionFinal
, stdenv
, writeShellScriptBin
, bats
, nix
}:
let
  # self tests for holonix
  # mostly smoke tests on various platforms
  name = "hn-test";

  script = writeShellScriptBin name ''
    set -e

    bats ./test/clippy.bats
    # TODO: revisit when decided on a new gihtub-release binary
    # bats ./test/github-release.bats

    bats ./test/nix-shell.bats
    ${if stdenv.isLinux then "bats ./test/perf.bats" else ""}
    bats ./test/rust-manifest-list-unpinned.bats
    bats ./test/rust.bats
    bats ./test/flamegraph.bats
    bats ./test/holochain-binaries.bats
    ${if (isIncludedFn "launcher" && (holochainVersionFinal.launcher or null)
      != null) then
      "bats ./test/launcher.bats"
    else
      ""}
    ${if (isIncludedFn "scaffolding"
      && (holochainVersionFinal.scaffolding or null) != null) then
      "bats ./test/scaffolding.bats"
    else
      ""}
  '';

in
{
  buildInputs = [
    script
    # test system for bash
    # https://github.com/sstephenson/bats
    bats

    nix
  ];
}
