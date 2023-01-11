self: super:

{
  rust = super.rust // ({
    defaultExtensions = [ "rust-src" ];

    defaultTargets = [
      "aarch64-unknown-linux-musl"
      "wasm32-unknown-unknown"
      "x86_64-pc-windows-gnu"
      "x86_64-unknown-linux-musl"
      "x86_64-apple-darwin"
    ];

    mkRust = { track, version }:
      (self.rust-bin."${track}"."${version}".default.override {
        extensions = self.rust.defaultExtensions;
        targets = self.rust.defaultTargets;
      }).overrideAttrs (_: { passthru = { inherit (self) llvmPackages; }; });

    rustNightly = self.rust.mkRust {
      track = "nightly";
      version = "latest";
    };
    rustStable = self.rust.mkRust {
      track = "stable";
      version = "1.64.0";
    };
    rustHolochain = self.rust.mkRust {
      track = "stable";
      version = "1.64.0";
    };

    packages = super.rust.packages // {
      nightly = {
        rustPlatform = self.makeRustPlatform {
          rustc = self.rust.rustNightly;
          cargo = self.rust.rustNightly;
        };

        inherit (self.rust.packages.nightly.rustPlatform) rust;
      };

      stable = {
        rustPlatform = self.makeRustPlatform {
          rustc = self.rust.rustStable;
          cargo = self.rust.rustStable;
        };

        inherit (self.rust.packages.stable.rustPlatform) rust;
      };

      holochain = {
        rustPlatform = self.makeRustPlatform {
          rustc = self.rust.rustHolochain;
          cargo = self.rust.rustHolochain;
        };

        inherit (self.rust.packages.holochain.rustPlatform) rust;
      };

    };
  });

  inherit (self.rust.packages.stable.rust) rustc cargo;
}
