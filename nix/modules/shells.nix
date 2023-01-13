{ self, lib, inputs, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }: let
    hcMkShell = input: let
      shell = pkgs.mkShell {
        # mkShell reverses the inputs list, which breaks order-sensitive shellHooks
        inputsFrom = lib.reverseList [
          { shellHook = "source ${self'.packages.nixEnvPrefixEval}"; }

          /*
            To reagain all attributes from the original holonix shell, more
              more of holonix needs to be migrated into this repo.
            For now, this is OK, because `nix-shell` still uses the old holonix
              shell definitions and not the ones defined here.
          */
          # holonix.main

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
      });

  in {
    # shell for HC core development. included dependencies:
    # * everything needed to compile this repos' crates
    # * CI scripts
    devShells.coreDev = hcMkShell {
      buildInputs =
        (builtins.attrValues (config.coreScripts))
        ++ (with pkgs;[
          cargo-nextest
          sqlcipher
          gdb
          gh
          nixpkgs-fmt
          cargo-sweep
        ])
        ++ [
          config.crate2nix
        ];
    };

    devShells.release = self'.devShells.coreDev.overrideAttrs (attrs: {
      nativeBuildInputs = attrs.nativeBuildInputs ++ (with pkgs; [
        niv
        cargo-readme
        (import (self + /crates/release-automation/default.nix) {
          inherit pkgs;
        })
      ]);
    });

    devShells.ci = hcMkShell {
      inputsFrom = [
        (builtins.removeAttrs self'.devShells.coreDev [ "shellHook" ])
      ];
      nativeBuildInputs = (with self'.packages; [
        ciSetupNixConf
        ciCachixPush
      ]);
    };

    devShells.happDev = hcMkShell {
      inputsFrom = [
        (builtins.removeAttrs self'.devShells.coreDev [ "shellHook" ])
      ];
      nativeBuildInputs =
        (with self'.packages; [
          happ-holochain
          happ-hc
        ])
        ++ (with pkgs; [
          sqlcipher
          binaryen
          gdb
        ]);
    };

    devShells.coreDevRustup = self'.devShells.coreDev.overrideAttrs (attrs: {
      buildInputs = attrs.buildInputs ++ (with pkgs; [
        rustup
      ]);
    });
  };
}

