{
  description = "The new, performant, and simplified version of Holochain on Rust (sometimes called Holochain RSM for Refactored State Model) ";

  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nix-filter.url = "github:numtide/nix-filter";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
    crate2nix = {
      url = "github:kolloch/crate2nix";
      flake = false;
    };
    # eventually this should be removed, but `nix-shell` still requires holonix.
    holonix = {
      url = "https://github.com/holochain/holonix";
      flake = false;
    };
  };

  outputs = inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        ./nix/modules/srcCleaned.nix
        ./nix/modules/core.nix
        ./nix/modules/coreScripts.nix
        ./nix/modules/crate2nix.nix
        ./nix/modules/hnRustClippy.nix
        ./nix/modules/hnRustFmtCheck.nix
        ./nix/modules/hnRustFmtFmt.nix
        ./nix/modules/holochain.nix
        ./nix/modules/holochain-crate2nix.nix
        ./nix/modules/nixEnvPrefixEval.nix
        ./nix/modules/releaseAutomation.nix
        ./nix/modules/rust
        ./nix/modules/shells
      ];
      systems = [ "x86_64-linux" "x86_64-darwin" "aarch64-darwin" ];
      perSystem = { config, self', inputs', ... }: {
        # Per-system attributes can be defined here. The self' and inputs'
        # module parameters provide easy access to attributes of the same
        # system.
        devShells.default = self'.devShells.coreDev;

      };
      flake = {
        # The usual flake attributes can be defined here, including system-
        # agnostic ones like nixosModule and system-enumerating ones, although
        # those are more easily expressed in perSystem.

      };
    };
}
