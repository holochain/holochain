{ self, lib, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }: {
    devShells = {
      default = self'.devShells.holonix;

      holonix = pkgs.mkShell {
        inputsFrom = [ self'.devShells.rustDev ];

        packages = with self'.packages; [ holochain lair-keystore hc-launch hc-scaffold ];
      };

      release = pkgs.mkShell {
        inputsFrom = [ self'.devShells.coreDev ];

        packages = (with self'.packages; [ release-automation cargo-rdme ])
          ++ (with pkgs; [ cargo-readme cargo-sweep gh gitFull ]);
      };

      coreDev = pkgs.mkShell {
        inputsFrom = [ self'.devShells.rustDev ];

        packages = with pkgs; [ cargo-nextest ];

        shellHook = ''
          export HC_TEST_WASM_DIR="$CARGO_TARGET_DIR/.wasm_target"
          mkdir -p $HC_TEST_WASM_DIR

          export HC_WASM_CACHE_PATH="$CARGO_TARGET_DIR/.wasm_cache"
          mkdir -p $HC_WASM_CACHE_PATH
        '';
      };

      rustDev = pkgs.mkShell
        {
          inherit (self'.packages.holochain) nativeBuildInputs;
          shellHook = ''
            export CARGO_HOME="$PWD/.cargo"
            export CARGO_INSTALL_ROOT="$PWD/.cargo"
            export CARGO_TARGET_DIR="$PWD/target"
            export CARGO_CACHE_RUSTC_INFO=1
            export PATH="$CARGO_INSTALL_ROOT/bin:$PATH"
            export NIX_PATH="nixpkgs=${pkgs.path}"
          '' + (lib.strings.optionalString pkgs.stdenv.isDarwin ''
            export DYLD_FALLBACK_LIBRARY_PATH="$(rustc --print sysroot)/lib"
          '');
        };
    };
  };
}
