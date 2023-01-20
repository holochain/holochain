{ self, lib, ... }: {
  options.sources = lib.mkOption {type = lib.types.raw;};
  config.sources = let
    sourcesJSON =
      builtins.fromJSON (builtins.readFile (self + /nix/sources.json));

    holonix = builtins.fetchTarball {
      inherit (sourcesJSON.holonix) url sha256;
    };

    holonixSourcesJSON =
      builtins.fromJSON (builtins.readFile ("${holonix}/nix/sources.json"));

    holochain-nixpkgs = builtins.fetchTarball {
      inherit (holonixSourcesJSON.holochain-nixpkgs) url sha256;
    };

    holochainNixpkgsSourcesJSON =
      builtins.fromJSON (
        builtins.readFile
        ("${holochain-nixpkgs}/nix/nvfetcher/_sources/generated.json")
      );

    nixpkgs = builtins.fetchTarball {
      inherit (holochainNixpkgsSourcesJSON.nixpkgs.src) url sha256;
    };
  in {
    inherit
      holonix
      holochain-nixpkgs
      nixpkgs
      ;
  };
}
