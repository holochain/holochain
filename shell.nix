{ ... }:

let 
  default = import (builtins.toString ./default.nix);
  holonix = default.holonix;

  pwd = builtins.toString ./.;

  shell = holonix.pkgs.stdenv.mkDerivation (holonix.shell // {
    name = "dev-shell";

    shellHook = holonix.pkgs.lib.concatStrings [
      holonix.shell.shellHook
      ''
       touch .env
       source .env

       export HC_TARGET_PREFIX=$NIX_ENV_PREFIX
       export CARGO_TARGET_DIR="$HC_TARGET_PREFIX/target"
       export HC_TEST_WASM_DIR="$HC_TARGET_PREFIX/.wasm_target"
       mkdir -p $HC_TEST_WASM_DIR
       export CARGO_CACHE_RUSTC_INFO=1

       export HC_WASM_CACHE_PATH="$HC_TARGET_PREFIX/.wasm_cache"
       mkdir -p $HC_WASM_CACHE_PATH

       export PEWPEWPEW_PORT=4343
      ''
    ];

    buildInputs = with holonix.pkgs; [
        gnuplot
        flamegraph
        fd
        ngrok
        jq
      ]
      ++ holonix.shell.buildInputs
      ++ (holonix.pkgs.lib.attrsets.mapAttrsToList (name: value: value.buildInputs) default.pkgs)
    ;
  });

in shell
