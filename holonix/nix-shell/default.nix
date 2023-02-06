{ stdenv
, mkShell
, bashInteractive
, coreutils
, flamegraph
, nix

, holochain-nixpkgs
, holonixComponents
, holonixVersions
}:

let
  base = {

    shellHook = ''
      # cargo should install binaries into this repo rather than globally
      # https://github.com/rust-lang/rustup.rs/issues/994
      #
      # cargo should NOT install binaries into this repo in vagrant as this breaks
      # under windows with virtualbox shared folders

      if [[ -z $NIX_ENV_PREFIX ]]
      then
       if [[ $( whoami ) == "vagrant" ]]
        then export NIX_ENV_PREFIX=/home/vagrant
        else export NIX_ENV_PREFIX=`pwd`
       fi
      fi

      export CARGO_HOME="$NIX_ENV_PREFIX/.cargo"
      export CARGO_INSTALL_ROOT="$NIX_ENV_PREFIX/.cargo"
      export CARGO_TARGET_DIR="$NIX_ENV_PREFIX/target"
      export CARGO_CACHE_RUSTC_INFO=1
      export PATH="$CARGO_INSTALL_ROOT/bin:$PATH"
      export NIX_BUILD_SHELL="${bashInteractive}/bin/bash"
      export NIX_PATH="nixpkgs=${holochain-nixpkgs.pkgs.path}"

      # https://github.com/holochain/holonix/issues/12
      export TMP=$( mktemp -p /tmp -d )
      export TMPDIR=$TMP

    '';

    buildInputs = [
      nix

      # for mktemp
      coreutils

      #flame graph dep
      flamegraph

      holonixVersions
    ];
  };

in
(mkShell {
  name = "holonix-shell";

  inputsFrom = holonixComponents ++
    # this list is reversed [0] and we want the base shell to be first as the shellHook sets the NIX_ENV_PREFIX
    # [0]: https://github.com/NixOS/nixpkgs/blob/966a7403df58a4a72295bce08414de90bb80bbc6/pkgs/build-support/mkshell/default.nix#L42
    [ base ];
}).overrideAttrs (attrs: {
  nativeBuildInputs =
    builtins.filter (el: (builtins.match ".*(rust|cargo).*" el.name) == null)
      attrs.nativeBuildInputs;
})
