{ lib
, runCommand
, openssl
, pkg-config

, gnutar
, hcRustPlatform
}:

let
  cratesStoreDir = ../../crates;

  holochainSrc = builtins.path {
    path = ../..;
    name = "holochain-src";
    filter = path: type:
      (builtins.match "(.*/Cargo.toml|.*/Cargo.lock|.*/crates.*)" path) != null
      ;
    recursive = true;
  };

  holochainBinaries = { package, additionalCargoBuildFlags ? [] }: hcRustPlatform.buildRustPackage {
    name = "holochain-binaries";

    src = holochainSrc;
    cargoSha256 = "0hfbq1mcpbv620zxsxfaxn9mjffjshrxmp50d41ql5jxykl1197m";

    nativeBuildInputs = [
      pkg-config
    ];

    buildInputs = [
      openssl.dev
    ];

    cargoBuildFlags = [
      "--manifest-path=crates/${package}/Cargo.toml"
      ]
      ++ additionalCargoBuildFlags
      ;

    # buildAndTestSubdir = "crates/${package}";
    doCheck = false;

    meta = with lib; {
      description = "Holochain, a framework for distributed applications";
      homepage = "https://github.com/holochain/holochain";
      license = licenses.cpal10;
      maintainers = [ "Holochain Core Dev Team <devcore@holochain.org>" ];
    };
  };

  mkHolochainBinary = { package, binaries ? [ package ], additionalCargoBuildFlags ? [] }:
    let
      manifest = builtins.fromTOML (builtins.readFile (builtins.concatStringsSep "/" [
        cratesStoreDir
        package
        "/Cargo.toml"
      ]));
      version = manifest.package.version;
      binary = holochainBinaries { inherit package additionalCargoBuildFlags; };
    in runCommand "${package}-${version}" { inherit (binary) buildInputs nativeBuildInputs; } ''
      mkdir $out
      for bin in ${builtins.concatStringsSep " " binaries}; do
        ln -s ${binary}/bin/$bin $out/$bin
      done
    '';

in
{
  holochain = mkHolochainBinary {
    package = "holochain";
    additionalCargoBuildFlags = [ "--no-default-features" ];
  };

  dnaUtil = mkHolochainBinary {
    package = "dna_util";
    binaries = [ "dna-util" ];
  };
}
