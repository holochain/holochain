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

        export HC_TEST_WASM_DIR="$CARGO_TARGET_DIR/.wasm_target"
        mkdir -p $HC_TEST_WASM_DIR

        export HC_WASM_CACHE_PATH="$CARGO_TARGET_DIR/.wasm_cache"
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
    nativeBuildInputs = builtins.attrValues (pkgs.core)
      ++ [ holonix.pkgs.sqlcipher ];
  };

  ci = hcMkShell {
    inputsFrom = [
      (builtins.removeAttrs coreDev [ "shellHook" ])
    ];
    nativeBuildInputs = builtins.attrValues pkgs.ci;
  };

  happDev = hcMkShell {
    inputsFrom = [
      (builtins.removeAttrs coreDev [ "shellHook" ])
    ];
    nativeBuildInputs = builtins.attrValues pkgs.happ
      ++ (with holonix.pkgs; [
        lair-keystore
        sqlcipher
        binaryen
      ])
      ;
  };

  coreDevRustup = coreDev.overrideAttrs (attrs: {
    buildInputs = attrs.buildInputs ++ [
      rustup
    ];
  });
}
