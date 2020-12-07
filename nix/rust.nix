{ callPackage
, fetchFromGitHub
, makeRustPlatform
, rustChannelOf

, version ? "1.45.2"
, targets ? [ "wasm32-unknown-unknown" ]
}:

let
  hcRust = (rustChannelOf { channel = version; }).rust.override {
    inherit targets;
  };
in

{
  inherit hcRust;

  hcRustPlatform = makeRustPlatform {
    cargo = hcRust;
    rustc = hcRust;
  };
}
