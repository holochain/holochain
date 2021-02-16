{ callPackage
, hcRustPlatform
, writeShellScriptBin

, hcToplevelDir
, nixEnvPrefixEval
}:

let
  hcRunCrate = writeShellScriptBin "hc-run-crate" ''
    set -x
    ${nixEnvPrefixEval}

    crate=''${1:?The first argument needs to define the crate name}
    shift
    cargo run --target-dir=''${CARGO_TARGET_DIR:?} --manifest-path=${hcToplevelDir}/crates/$crate/Cargo.toml -- $@
  '';

  ci = callPackage ./ci.nix { };
  core = callPackage ./core.nix { };
  happ = let
    mkHolochainBinaryScript = crate: writeShellScriptBin (builtins.replaceStrings ["_"] ["-"] crate) ''
      exec ${hcRunCrate}/bin/hc-run-crate ${crate} $@
    '';
  in {
    holochain = mkHolochainBinaryScript "holochain";
    dnaUtil = mkHolochainBinaryScript "dna_util";
  };

  all = {
    inherit
      core
      ci
      happ
      ;
  };

in builtins.mapAttrs (k: v:
  builtins.removeAttrs v [ "override" "overrideDerivation" ]
) all
