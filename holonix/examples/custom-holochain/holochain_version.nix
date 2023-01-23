# This file was generated with the following command:
# update-holochain-versions --git-src=revision:holochain-0.0.129 --lair-version-req=~0.0 --output-file=holochain_version.nix
# For usage instructions please visit https://github.com/holochain/holochain-nixpkgs/#readme

{
  url = "https://github.com/holochain/holochain";
  rev = "holochain-0.0.129";
  sha256 = "sha256-Mtp9fI71JqM/Qa3wsUvwkGlQdVQH3vOdD7jtYaqVdbg=";
  cargoLock = { outputHashes = { }; };

  binsFilter = [ "holochain" "hc" "kitsune-p2p-proxy" "kitsune-p2p-tx2-proxy" ];

  rustVersion = "1.58.1";

  lair = {
    url = "https://github.com/holochain/lair";
    rev = "v0.0.9";
    sha256 = "sha256-glSixh2GtWtJ1wswAA0Q2hnLIFPQY+Tsh36IcUgIbRs=";

    binsFilter = [ "lair-keystore" ];

    rustVersion = "1.58.1";

    cargoLock = { outputHashes = { }; };
  };
}
