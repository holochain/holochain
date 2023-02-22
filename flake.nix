{
  description =
    "Holochain is an open-source framework to develop peer-to-peer applications with high levels of security, reliability, and performance.";

  inputs = {
    # nix packages pointing to the github repo
    nixpkgs.url = "nixpkgs/nixos-unstable";

    # lib to build nix packages from rust crates
    crate2nix = {
      url = "github:kolloch/crate2nix";
      flake = false;
    };

    # lib to build nix packages from rust crates
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # filter out all .nix files to not affect the input hash
    # when these are changes
    nix-filter.url = "github:numtide/nix-filter";
    # provide downward compatibility for nix-shell/derivation users
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };

    # To execute checks when making a commit
    # Only /flake-module.nix is needed here -> Importing with `flake=false`.
    pre-commit-hooks-nix.url = "github:cachix/pre-commit-hooks.nix";
    pre-commit-hooks-nix.flake = false;

    # rustup, rust and cargo
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    versions = {
      url = "github:holochain/holochain?dir=versions/0_1";
    };

    holochain.follows = "versions/holochain";
    holochain.flake = false;
    lair.follows = "versions/lair";
    lair.flake = false;
    launcher.follows = "versions/launcher";
    launcher.flake = false;
    scaffolding.follows = "versions/scaffolding";
    scaffolding.flake = false;

    cargo-chef = {
      url = "github:LukeMathWalker/cargo-chef/main";
      flake = false;
    };

    cargo-rdme = {
      url = "github:orium/cargo-rdme/v1.1.0";
      flake = false;
    };
  };

  # refer to flake-parts docs https://flake.parts/
  outputs = inputs @ { self, nixpkgs, flake-parts, ... }:
    # all possible parameters for a module: https://flake.parts/module-arguments.html#top-level-module-arguments
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "aarch64-darwin" "x86_64-linux" "x86_64-darwin" "aarch64-linux" ];

      imports =
        # auto import all nix code from `./modules`, treat each one as a flake and merge them
        (
          map (m: "${./.}/nix/modules/${m}")
            (builtins.attrNames (builtins.readDir ./nix/modules))
        )
        ++ [
          (inputs.pre-commit-hooks-nix + /flake-module.nix)
        ];

      perSystem = { pkgs, ... }: {
        legacyPackages = pkgs;
      };
    };
}
