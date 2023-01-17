{
  lib,
  pkgs,
  nixEnvPrefixEval,
}:
input: let
  shell = pkgs.mkShell {
    # mkShell reverses the inputs list, which breaks order-sensitive shellHooks
    inputsFrom = lib.reverseList [
      {

        shellHook = ''
          source ${nixEnvPrefixEval}

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
          export NIX_BUILD_SHELL="${pkgs.bashInteractive}/bin/bash"
          export NIX_PATH="nixpkgs=${pkgs.path}"

          # https://github.com/holochain/holonix/issues/12
          export TMP=$( mktemp -p /tmp -d )
          export TMPDIR=$TMP

        '';

        buildInputs = (with pkgs; [
          nix
          # for mktemp
          coreutils

          #flame graph dep
          flamegraph
        ]);
      }

      {
        shellHook = ''
          >&2 echo Using "$NIX_ENV_PREFIX" as target prefix...

          export HC_TEST_WASM_DIR="$CARGO_TARGET_DIR/.wasm_target"
          mkdir -p $HC_TEST_WASM_DIR

          export HC_WASM_CACHE_PATH="$CARGO_TARGET_DIR/.wasm_cache"
          mkdir -p $HC_WASM_CACHE_PATH
        ''
        # workaround to make cargo-nextest work on darwin
        # see: https://github.com/nextest-rs/nextest/issues/267
        + (lib.strings.optionalString pkgs.stdenv.isDarwin ''
          export DYLD_FALLBACK_LIBRARY_PATH="$(rustc --print sysroot)/lib"
        '')
        ;
      }

      input
    ];
  };
in
  shell.overrideAttrs (attrs: {
    nativeBuildInputs =
      builtins.filter (el: (builtins.match ".*(rust|cargo).*" el.name) == null)
      attrs.nativeBuildInputs;
  })
