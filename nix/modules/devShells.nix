{ self, lib, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }:
    let
      holonixPackages = with self'.packages; [ holochain lair-keystore hc-launch hc-scaffold ];
      versionsFileText = builtins.concatStringsSep "\n"
        (
          builtins.map
            (package: ''
              echo ${package.pname} \($(${package}/bin/${package.pname} -V)\): ${package.src.rev or "na"}'')
            holonixPackages
        );
      hn-introspect =
        pkgs.writeShellScriptBin "hn-introspect" versionsFileText;
    in
    {
      packages = {
        inherit hn-introspect;
      };

      devShells = {
        default = self'.devShells.holonix;
        holonix = pkgs.mkShell {
          inputsFrom = [ self'.devShells.rustDev ];
          packages = holonixPackages ++ [ hn-introspect ];
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

        coreDev =
          let
            holochainTestDrvs =
              (
                lib.attrsets.filterAttrs
                  (name: package:
                    # (package.checkPhase or null) != null &&
                    (builtins.match "^build-holochain-tests.*" name) != null
                  )
                  self'.packages
              );

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
                      cleanCmd = cmd:
                        (builtins.replaceStrings
                          [ "cargo --version" "cargoWithProfile" "runHook preCheck" "runHook postCheck" "runHook preBuild" "runHook postBuild" "\n" "\\" "  " ]
                          [ "" "cargo" "" "" "" "" "" "" " " ]
                          cmd);

                      checkPhaseClean = cleanCmd (package.checkPhase or "");
                      buildPhaseClean = cleanCmd (package.buildPhase or "");
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
          pkgs.mkShell {
            inputsFrom = [ self'.devShells.rustDev ] ++ (builtins.attrValues holochainTestDrvs);

            packages = with pkgs; [
              cargo-nextest

              (pkgs.writeShellScriptBin "script-cargo-regen-lockfiles" ''
                cargo fetch --locked
                cargo generate-lockfile --offline --manifest-path=crates/test_utils/wasm/wasm_workspace/Cargo.toml
                cargo generate-lockfile --offline
                cargo generate-lockfile --offline --manifest-path=crates/test_utils/wasm/wasm_workspace/Cargo.toml
              '')

            ]

            # generate one script for each of the "holochain-tests-" prefixed derivations by reusing their checkPhase
            ++ builtins.attrValues (builtins.mapAttrs mkTestScript holochainTestDrvs)
            ;

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

        rustDev =
          pkgs.mkShell
            {
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


