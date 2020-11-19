{ callPackage
, fetchFromGitHub
, makeRustPlatform

, version ? "1.45.2"
, targets ? [ "wasm32-unknown-unknown" ]
}:

let
  mozillaOverlay = fetchFromGitHub {
    owner = "mozilla";
    repo = "nixpkgs-mozilla";
    # Updated 2020/12/02
    rev = "8c007b60731c07dd7a052cce508de3bb1ae849b4";
    sha256 = "1zybp62zz0h077zm2zmqs2wcg3whg6jqaah9hcl1gv4x8af4zhs6";
  };
  mozilla = callPackage "${mozillaOverlay.out}/package-set.nix" {};
in

rec {
  hcRust = (mozilla.rustChannelOf { channel = version; }).rust.override {
    inherit targets;
  };

  hcRustPlatform = makeRustPlatform {
    cargo = hcRust;
    rustc = hcRust;
  };
}
