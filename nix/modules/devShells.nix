{ self, lib, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }:
    let
      holonixPackages = with self'.packages; [ holochain lair-keystore hc-launch hc-scaffold ];

      holochainTestScripts =
        builtins.attrValues (builtins.mapAttrs mkTestScript holochainTestDrvs);

      holochainTestDrvs =
        lib.filterAttrs
          (name: package: lib.hasPrefix "build-holochain-tests." name)
          self'.packages;

      # TODO: improve separations of concerns
      #   cleanPhase and mkTestScript are strongly coupled with implementation
      #   details of ./holochain.nix.
      #   Changes in holochain.nix will likely break things here.
      #   Exporting a derivations build logic as a script should be of concern
      #   of that particular derivation and does not belong in devShells.nix.
      #   After refactoring this, most of the re-naming, filtering, conditionals
      #   and error handling won't be needed anymore leading to a cleaner
      #   implementation which is easier to review and maintain.
      cleanPhase = cmd:
        (builtins.replaceStrings
          [ "cargo --version" "cargoWithProfile" "runHook preCheck" "runHook postCheck" "runHook preBuild" "runHook postBuild" "\n" "\\" "  " ]
          [ "" "cargo" "" "" "" "" "" "" " " ]
          cmd);
      mkTestScript = name: package:
        pkgs.writeShellScriptBin (builtins.replaceStrings [ "build-" ] [ "script-" ] name)
          (
            ''
              set -xue
            ''
            # remove the craneLib internals that are part of the checkPhase and
            # some characters that would prevent the passing of args
            + (
              let
                checkPhaseClean = cleanPhase (package.checkPhase or "");
                buildPhaseClean = cleanPhase (package.buildPhase or "");
                checkOrBuildPhase =
                  if checkPhaseClean != "" then checkPhaseClean
                  else buildPhaseClean;
              in
              if checkOrBuildPhase != "" then
                ''${checkOrBuildPhase} ''${@}''
              else if (package.passthru.dependencies or null) != null then
              # recursive call to generate one script call per dependency
                builtins.concatStringsSep "\n" (builtins.map (pkg: "${mkTestScript pkg.name pkg}/bin/${pkg.name}") package.passthru.dependencies)
              else
                throw ''${name} has neither of these
                      - checkPhase: (${checkPhaseClean})
                      - buildPhase: (${buildPhaseClean})
                      - passthru.dependencies
                    ''
            )
          );

    in
    {
      devShells = {

        default = self'.devShells.holonix;

        holonix = pkgs.mkShell {
          inputsFrom = [ self'.devShells.rustDev ];
          packages = holonixPackages ++ [ self'.packages.hn-introspect ];
          shellHook = ''
            echo Holochain development shell spawned. Type 'exit' to leave.
            export PS1='\n\[\033[1;34m\][holonix:\w]\$\[\033[0m\] '
          '';
        };

        release = pkgs.mkShell {
          inputsFrom = [ self'.devShells.rustDev ];

          packages = (with self'.packages;
            [ release-automation cargo-rdme ])
          ++ (with pkgs; [ cargo-readme cargo-sweep gh gitFull cacert ]);
        };

        coreDev = pkgs.mkShell {
          inputsFrom = [ self'.devShells.rustDev ] ++ (builtins.attrValues holochainTestDrvs);

          packages =
            [
              pkgs.cargo-nextest
              self'.packages.script-cargo-regen-lockfiles
            ]
            # generate one script for each of the "holochain-tests-" prefixed derivations by reusing their checkPhase
            ++ holochainTestScripts;

          shellHook = ''
            export PS1='\n\[\033[1;34m\][coreDev:\w]\$\[\033[0m\] '

            export HC_TEST_WASM_DIR="$CARGO_TARGET_DIR/.wasm_target"
            mkdir -p $HC_TEST_WASM_DIR

            export HC_WASM_CACHE_PATH="$CARGO_TARGET_DIR/.wasm_cache"
            mkdir -p $HC_WASM_CACHE_PATH

            # Enables the pre-commit hooks
            ${config.pre-commit.installationScript}
          '';
        };

        rustDev = pkgs.mkShell {
          inputsFrom = [
            self'.packages.holochain
          ];

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


