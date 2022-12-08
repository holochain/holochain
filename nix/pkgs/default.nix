{ callPackage
, writeShellScriptBin

, hcToplevelDir
, nixEnvPrefixEval

, jq, util-linux
}:

let
  hcRunCrate = writeShellScriptBin "hc-run-crate" ''
    set -x
    ${nixEnvPrefixEval}

    crate=''${1:?The first argument needs to define the crate name}
    shift

    binary=$(cargo build \
      --locked \
      --target-dir=''${NIX_ENV_PREFIX:-?}/target \
      --manifest-path=${hcToplevelDir}/crates/$crate/Cargo.toml \
      --bin=$crate --message-format=json | \
        ${jq}/bin/jq \
          --slurp \
          --raw-output \
          'map(select(.executable != null))[0].executable' \
      )

    $binary $@
  '';

  hcCrateBinaryPath = writeShellScriptBin "hc-crate-binary-path" ''
    set -x
    ${nixEnvPrefixEval}

    crate=''${1:?The first argument needs to define the crate name}

    echo $(cargo build \
      --locked \
      --target-dir=''${NIX_ENV_PREFIX:-?}/target \
      --manifest-path=${hcToplevelDir}/crates/$crate/Cargo.toml \
      --bin=$crate --message-format=json | \
        ${jq}/bin/jq \
          --slurp \
          --raw-output \
          'map(select(.executable != null))[0].executable' \
      )
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
