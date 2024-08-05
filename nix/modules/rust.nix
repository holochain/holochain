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

      config.rustHelper = {
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

        mkRustPkgs =
          { track ? config.rustHelper.defaultTrack
          , version ? config.rustHelper.defaultVersion
          , extensions ? config.rustHelper.defaultExtensions
          , targets ? config.rustHelper.defaultTargets
          }:
          import inputs.nixpkgs {
            inherit system;

            overlays = [
              inputs.rust-overlay.overlays.default

              (final: prev: { })

              (final: prev: {
                rustToolchain =
                  (prev.rust-bin."${track}"."${version}".minimal.override ({
                    inherit extensions targets;
                  }));

                rustc = final.rustToolchain;
                cargo = final.rustToolchain;
              })
            ];
          };

        mkRust =
          { track ? config.rustHelper.defaultTrack
          , version ? config.rustHelper.defaultVersion
          , extensions ? config.rustHelper.defaultExtensions
          , targets ? config.rustHelper.defaultTargets
          }: (config.rustHelper.mkRustPkgs { inherit track version extensions targets; }).rustToolchain;

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
