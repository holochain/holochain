{ lib
, stdenv
, mkShell
, rustup
, coreutils

, holonix
, hcRustPlatform
, hcToplevelDir
, nixEnvPrefixEval
, pkgs
}:

let
  hcMkShell = input: mkShell {
    # mkShell reverses the inputs list, which breaks order-sensitive shellHooks
    inputsFrom = lib.reverseList [
      { shellHook = nixEnvPrefixEval; }

      holonix.shell

      { shellHook = ''
        echo Using "$NIX_ENV_PREFIX" as target prefix...

        export HC_TEST_WASM_DIR="$NIX_ENV_PREFIX/.wasm_target"
        mkdir -p $HC_TEST_WASM_DIR

        export HC_WASM_CACHE_PATH="$NIX_ENV_PREFIX/.wasm_cache"
        mkdir -p $HC_WASM_CACHE_PATH
      ''; }

      input
    ];
  };
in

rec {
  # shell for HC core development. included dependencies:
  # * everything needed to compile this repos' crates
  # * CI scripts
  coreDev = hcMkShell {
    nativeBuildInputs = builtins.attrValues (pkgs.core);
  };

  # we may need more packages on CI
  ci = coreDev;

  happDev = hcMkShell {
    inputsFrom = [
      (builtins.removeAttrs coreDev [ "shellHook" ])
    ];
    nativeBuildInputs = builtins.attrValues pkgs.happ;
  };

  coreDevRustup = coreDev.overrideAttrs (attrs: {
    buildInputs = attrs.buildInputs ++ [
      rustup
    ];
  });
}
