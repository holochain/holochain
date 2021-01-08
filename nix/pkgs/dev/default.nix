{ callPackage
, writeShellScriptBin

, hcToplevelDir
, hcTargetPrefixEval
, ...
}:

let
  hcRunCrate = writeShellScriptBin "hc-run-crate" ''
    set -x
    ${hcTargetPrefixEval}

    crate=''${1:?The first argument needs to define the crate name}
    shift
    cargo run --target-dir=''${HC_TARGET_PREFIX:?}/target --manifest-path=${hcToplevelDir}/crates/$crate/Cargo.toml -- $@
  '';

in

builtins.mapAttrs (k: v:
  builtins.removeAttrs v [ "override" "overrideDerivation" ]
) {
  core = callPackage ./core.nix { inherit hcRunCrate; };
  happ = callPackage ./happ.nix { inherit hcRunCrate; };
}
