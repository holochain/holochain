{ writeShellScriptBin
, hcRunCrate
}:

let
  mkHolochainBinaryScript = crate: writeShellScriptBin (builtins.replaceStrings ["_"] ["-"] crate) ''
    exec ${hcRunCrate}/bin/hc-run-crate ${crate} $@
  '';
in

{
  holochain = mkHolochainBinaryScript "holochain";
  dnaUtil = mkHolochainBinaryScript "dna_util";
}
