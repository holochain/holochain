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
    cargo run --target-dir=''${NIX_ENV_PREFIX:-?}/target/hc-run-crate --manifest-path=${hcToplevelDir}/crates/$crate/Cargo.toml -- $@
  '';

  mkHolochainBinaryScript = crate: writeShellScriptBin (builtins.replaceStrings ["_"] ["-"] crate) ''
    exec ${hcRunCrate}/bin/hc-run-crate ${crate} $@
  '';

  hcReleaseAutomation = writeShellScriptBin "hc-ra" ''
    exec ${hcRunCrate}/bin/hc-run-crate "release-automation" $@
  '';

  ci = callPackage ./ci.nix { };
  core = callPackage ./core.nix {
    inherit hcToplevelDir;
    releaseAutomation = "${hcReleaseAutomation}/bin/hc-ra";
  } // {
    inherit hcReleaseAutomation;
  };
  happ = {
    holochain = mkHolochainBinaryScript "holochain";
    hc = mkHolochainBinaryScript "hc";
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
