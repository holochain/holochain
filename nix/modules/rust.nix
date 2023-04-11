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
      options.rustHelper = lib.mkOption { type = lib.types.raw; };

      config.rustHelper = {
        defaultTrack = "stable";
        defaultVersion = "1.69.0";

        defaultExtensions = [ "rust-src" ];

        defaultTargets = [
          "aarch64-unknown-linux-musl"
          "wasm32-unknown-unknown"
          "x86_64-pc-windows-gnu"
          "x86_64-unknown-linux-musl"
          "x86_64-apple-darwin"
          "aarch64-apple-darwin"
        ];

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

              (final: prev: {
                rustToolchain =
                  (prev.rust-bin."${track}"."${version}".default.override ({
                    inherit extensions targets;
                  }));

                rustc = final.rustToolchain;
                cargo = final.rustToolchain;
              })

              (final: prev: {
                buildRustCrate = arg: prev.buildRustCrate (arg // {
                  dontStrip = prev.stdenv.isDarwin;
                });
              })

            ];
          };

        mkRust =
          { track ? config.rustHelper.defaultTrack
          , version ? config.rustHelper.defaultVersion
          }: (config.rustHelper.mkRustPkgs { inherit track version; }).rustToolchain;

        crate2nixTools = { pkgs }: import "${inputs.crate2nix}/tools.nix" {
          inherit pkgs;
        };
        customBuildRustCrateForPkgs = _: pkgs.buildRustCrate.override {
          defaultCrateOverrides = pkgs.lib.attrsets.recursiveUpdate pkgs.defaultCrateOverrides
            ({
              # this regular module named `build.rs` confuses crate2nix which tries to build and run it as a build script.
              build-fs-tree = _: {
                prePatch = ''
                  mv build.rs build/mod.rs
                '';
              };

              openssl-sys = _:
                let
                  opensslStatic = pkgs.pkgsStatic.openssl;
                in
                {
                  OPENSSL_NO_VENDOR = "1";
                  OPENSSL_LIB_DIR = "${opensslStatic.out}/lib";
                  OPENSSL_INCLUDE_DIR = "${opensslStatic.dev}/include";

                  nativeBuildInputs = [
                    pkgs.pkg-config
                  ];

                  buildInputs = [
                    pkgs.openssl
                    opensslStatic
                  ];
                };
              tx5-go-pion-sys = _: { nativeBuildInputs = with pkgs; [ go ]; };
              tx5-go-pion-turn = _: { nativeBuildInputs = with pkgs; [ go ]; };
              holochain = attrs: {
                codegenUnits = 8;
              };

              gobject-sys = _: { buildInputs = with pkgs; [ pkg-config glib ]; };
            });
        };


        mkCargoNix = { name, src, pkgs, buildRustCrateForPkgs ? config.rustHelper.customBuildRustCrateForPkgs }:
          let
            generated = (config.rustHelper.crate2nixTools {
              inherit pkgs;
            }).generatedCargoNix {
              inherit name src;
            };
          in
          pkgs.callPackage "${generated}/default.nix" { inherit buildRustCrateForPkgs; };


        # `nix flake show` is incompatible with IFD by default
        # This works around the issue by making the name of the package
        #   discoverable without IFD.
        mkNoIfdPackage = name: pkg: {
          inherit name;
          inherit (pkg) drvPath outPath;
          type = "derivation";
          orig = pkg;
        };

        apple_sdk =
          lib.attrsets.optionalAttrs pkgs.stdenv.isDarwin
            (
              # aarch64 only uses 11.0 and x86_64 mixes them
              if system == "x86_64-darwin"
              then pkgs.darwin.apple_sdk_10_12
              else pkgs.darwin.apple_sdk_11_0
            );
      };
    };
}
