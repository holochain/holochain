{ lib
, stdenv
, mkShell
, rustup
, coreutils
, cargo-nextest
, crate2nix

, holonix
, hcToplevelDir
, nixEnvPrefixEval
, pkgs
}:

let
  hcMkShell = input: mkShell {
    # mkShell reverses the inputs list, which breaks order-sensitive shellHooks
    inputsFrom = lib.reverseList [
      { shellHook = nixEnvPrefixEval; }

      holonix.main

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
        + (lib.strings.optionalString stdenv.isDarwin ''
          export DYLD_FALLBACK_LIBRARY_PATH="$(rustc --print sysroot)/lib"
        '')
        ;
      }

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
      ++ [
      cargo-nextest
      crate2nix
    ]
      ++ (with holonix.pkgs;[
      sqlcipher
      gdb
      gh
      nixpkgs-fmt
      cargo-sweep
    ]);
  };

  release = coreDev.overrideAttrs (attrs: {
    nativeBuildInputs = attrs.nativeBuildInputs ++ (with holonix.pkgs; [
      niv
      cargo-readme
      (import ../crates/release-automation/default.nix { })
    ]);
  });



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
      sqlcipher
      binaryen
      gdb
    ])
    ;
  };

  coreDevRustup = coreDev.overrideAttrs (attrs: {
    buildInputs = attrs.buildInputs ++ [
      rustup
    ];
  });
}
