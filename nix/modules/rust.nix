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
      options.rust = lib.mkOption { type = lib.types.raw; };
      config.rust =
        let
          rustPkgs = import pkgs.path {
            inherit system;
            overlays = [
              inputs.rust-overlay.overlays.default

              (self: super: {
                rust =
                  super.rust
                  // {
                    defaultExtensions = [ "rust-src" ];

                    defaultTargets = [
                      "aarch64-unknown-linux-musl"
                      "wasm32-unknown-unknown"
                      "x86_64-pc-windows-gnu"
                      "x86_64-unknown-linux-musl"
                      "x86_64-apple-darwin"
                    ];

                    mkRust =
                      { track
                      , version
                      ,
                      }: (self.rust-bin."${track}"."${version}".default.override {
                        extensions = self.rust.defaultExtensions;
                        targets = self.rust.defaultTargets;
                      });
                  };
              })
            ];
          };
        in
        rustPkgs.rust;
    };
}
