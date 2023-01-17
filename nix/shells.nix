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

  # monkey patching the old Shell as the plan is to get rid of holonix
  _coreDevOld = hcMkShell {
    nativeBuildInputs = builtins.attrValues (pkgs.core)
      ++ [
      cargo-nextest
    ]
      ++ (with holonix.pkgs;[
      sqlcipher
      gdb
      gh
      nixpkgs-fmt
      cargo-sweep
    ])
    # the latest crate2nix is currently broken on darwin
     ++ (lib.optionals stdenv.isLinux [
      crate2nix
    ]);
  };

  replacePackage = pkg: let
    pname = pkg.pname or null;
  in
    if pname == "nix"
    then pkgs.nix
    else pkg;

in

rec {
  # shell for HC core development. included dependencies:
  # * everything needed to compile this repos' crates
  # * CI scripts

  # monkey patching the old Shell as the plan is to get rid of holonix
  coreDev = _coreDevOld.overrideAttrs (old: {
    nativeBuildInputs = map replacePackage old.nativeBuildInputs;
  });

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
