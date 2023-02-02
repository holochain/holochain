{
  description =
    "The new, performant, and simplified version of Holochain on Rust (sometimes called Holochain RSM for Refactored State Model) ";

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
    # rustup, rust and cargo
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # lair
    lair = {
      url = "github:holochain/lair";
      flake = false;
    };

    # holochain_scaffolding_cli
    scaffolding = {
      url = "github:holochain/scaffolding";
      flake = false;
    };

    # launcher
    launcher = {
      url = "github:holochain/launcher";
      flake = false;
    };

    cargo-chef = {
      url = "github:LukeMathWalker/cargo-chef/main";
      flake = false;
    };
  };

  # refer to flake-parts docs https://flake.parts/
  outputs = inputs @ { self, nixpkgs, flake-parts, ... }:
    # all possible parameters for a module: https://flake.parts/module-arguments.html#top-level-module-arguments
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "aarch64-darwin" "x86_64-linux" "x86_64-darwin" ];

      # auto import all nix code from `./modules`, treat each one as a flake and merge them
      imports = map (m: "${./.}/nix/modules/${m}")
        (builtins.attrNames (builtins.readDir ./nix/modules));
    };
}
