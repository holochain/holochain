{ self
, lib
, inputs
, ...
} @ flake: {
  perSystem =
    { config
    , self'
    , inputs'
    , system
    , pkgs
    , ...
    }: {
      config.packages = {
        opensslStatic =
          if system == "x86_64-darwin"
          then pkgs.openssl # pkgsStatic is considered a cross build
          # and this is not yet supported
          else pkgs.pkgsStatic.openssl;
      };

      options.rustHelper = lib.mkOption { type = lib.types.raw; };

      config.rustHelper =
        let
          rustPkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ (import inputs.rust-overlay) ];
          };
        in
        {
          defaultTrack = "stable";
          defaultVersion = "1.77.2";

          defaultExtensions = [
            "rust-src"
            "rust-analyzer"
            "clippy"
            "rustfmt"
          ];

          defaultTargets = [
            "wasm32-unknown-unknown"
          ];

          defaultStdenv = pkgs:
            if pkgs.stdenv.isLinux
            then pkgs.stdenvAdapters.useMoldLinker pkgs.stdenv
            else pkgs.stdenv;

          mkRust =
            { track ? config.rustHelper.defaultTrack
            , version ? config.rustHelper.defaultVersion
            , extensions ? config.rustHelper.defaultExtensions
            , targets ? config.rustHelper.defaultTargets
            }: (rustPkgs.rust-bin."${track}"."${version}".minimal.override ({
              inherit extensions targets;
            }));

          customBuildRustCrateForPkgs = pkgs: pkgs.buildRustCrate.override {
            stdenv = config.rustHelper.defaultStdenv pkgs;

            defaultCrateOverrides = pkgs.lib.attrsets.recursiveUpdate pkgs.defaultCrateOverrides
              ({
                tx5-go-pion-sys = _: { nativeBuildInputs = with pkgs; [ go ]; };
                tx5-go-pion-turn = _: { nativeBuildInputs = with pkgs; [ go ]; };
              });
          };

          # `nix flake show` is incompatible with IFD by default
          # This works around the issue by making the name of the package
          #   discoverable without IFD.
          mkNoIfdPackage = name: pkg: {
            inherit name;
            inherit (pkg) drvPath outPath;
            type = "derivation";
            orig = pkg;
          };
        };
    };
}
