# This is the default nix file FOR HOLONIX
# This file is what nix will find when hitting this repo as a tarball
# This means that downstream consumers should pkgs.callPackage this file
# See example.default.nix for an example of how to consume this file downstream
{
# allow consumers to pass in their own config
# fallback to empty sets
config ? import ./config.nix
, holochain-nixpkgs ? config.holochain-nixpkgs.importFn { }
, includeHolochainBinaries ? include.holochainBinaries or true
, include ? { test = false; }, isIncludedFn ? (name: include."${name}" or true)

# either one listed in VERSIONS.md or "custom". when "custom" is set, `holochainVersion` needs to be specified
, holochainVersionId ? "v0_1_3", holochainVersion ? null, rustVersion ? { }
, rustc ? (if rustVersion == { } then
  holochain-nixpkgs.pkgs.rust.packages.stable.rust.rustc
else
  holochain-nixpkgs.pkgs.rust.mkRust ({
    track = "stable";
    version = "latest";
  } // (if rustVersion != null then rustVersion else { }))), inNixShell ? false
}:

let
  holochainVersionFinal = if holochainVersionId == "custom" then
    if holochainVersion == null then
      throw ''
        When 'holochainVersionId' is set to "custom" a value to 'holochainVersion' must be provided.''
    else
      holochainVersion
  else
    (let
      value' = builtins.getAttr holochainVersionId
        holochain-nixpkgs.packages.holochain.holochainVersions;
      value = (value' // {
        scaffolding = if isIncludedFn "scaffolding" == true then
          (value'.scaffolding or null)
        else
          null;
        launcher = if isIncludedFn "launcher" == true then
          (value'.launcher or null)
        else
          null;
      });

    in if holochainVersion != null then
      builtins.trace ''
        WARNING: ignoring the value of `holochainVersion` because `holochainVersionId` is not set to "custom"''
      value
    else
      value);

  sources = import nix/sources.nix { };

in assert (holochainVersionId == "custom") -> (let
  deprecatedAttributes = builtins.filter
    (elem: builtins.elem elem [ "cargoSha256" "bins" "lairKeystoreHashes" ])
    (builtins.attrNames holochainVersionFinal);

in if [ ] != deprecatedAttributes then
  (let holonixPath = builtins.toString ./.;

  in throw ''
    The following attributes found in the 'holochainVersion' set are no longer supported:
    ${builtins.concatStringsSep ", " deprecatedAttributes}

    The structure of 'holochainVersion' changed in a breaking way,
    and more supported values were added to 'holochainVersionId'.

    Please see if a matching 'holochainVersionId' for your desired version already exists:
    - ${holonixPath}/VERSIONS.md

    If not please take a look at the updated readme and example files for custom holochain versions:
    - ${holonixPath}/examples/custom-holochain

    If you're in a hurry you can rollback to holonix revision
    d326ee858e051a2525a1ddb0452cab3085c4aa98 or before.
  '')
else
  true);

let
  pkgs = import holochain-nixpkgs.pkgs.path {
    overlays = (builtins.attrValues holochain-nixpkgs.overlays) ++ [
      (self: super: {
        custom_rustc = rustc;

        holonix = ((import <nixpkgs> { }).callPackage or self.callPackage)
          ./pkgs/holonix.nix { inherit holochainVersionId holochainVersion; };
        holonixIntrospect = self.callPackage ./pkgs/holonix-introspect.nix {
          inherit (self) holochainBinaries;
        };

        holonixVersions = self.callPackage ./pkgs/holonix-versions.nix { };

        # these are referenced in holochain-s merge script.
        # ideally we'd expose all packages in this repository in this way.
        hnRustClippy =
          builtins.elemAt (self.callPackage ./rust/clippy { }).buildInputs 0;
        hnRustFmtCheck =
          builtins.elemAt (self.callPackage ./rust/fmt/check { }).buildInputs 0;
        hnRustFmtFmt =
          builtins.elemAt (self.callPackage ./rust/fmt/fmt { }).buildInputs 0;
        inherit holochainVersionId holochainVersionFinal;
        holochainBinaries =
          holochain-nixpkgs.packages.holochain.mkHolochainAllBinariesWithDeps
          holochainVersionFinal;
      })
    ];
  };

  components = {
    rust = pkgs.callPackage ./rust { inherit config rustc; };
    node = pkgs.callPackage ./node { };
    git = pkgs.callPackage ./git { };
    linux = pkgs.callPackage ./linux { };
    openssl = pkgs.callPackage ./openssl { };
    release = pkgs.callPackage ./release { config = config; };
    test = pkgs.callPackage ./test {
      inherit config isIncludedFn;

    };
    happs = pkgs.callPackage ./happs { };
    introspection = { buildInputs = [ pkgs.holonixIntrospect ]; };
    holochainBinaries = {
      buildInputs = (builtins.attrValues pkgs.holochainBinaries);
    };
    holochainDependencies = pkgs.mkShell {
      inputsFrom = (builtins.attrValues pkgs.holochainBinaries);
    };
    scaffolding = pkgs.callPackage ./mk-holochain-sub-binary {
      inherit sources;
      inherit (pkgs) holochainBinaries;
      name = "scaffolding";
    };
    launcher = pkgs.callPackage ./mk-holochain-sub-binary {
      inherit sources;
      inherit (pkgs) holochainBinaries;
      name = "launcher";
    };
    niv = { buildInputs = [ pkgs.niv ]; };
  };

  componentsFiltered =
    pkgs.lib.attrsets.filterAttrs (name: _value: isIncludedFn name) components;

  holonix-shell = pkgs.callPackage ./nix-shell {
    inherit holochain-nixpkgs;
    holonixComponents = builtins.attrValues componentsFiltered;
  };

  # override and overrideDerivation cannot be handled by mkDerivation
  derivation-safe-holonix-shell =
    (removeAttrs holonix-shell [ "override" "overrideDerivation" ]);

in {
  inherit holochain-nixpkgs pkgs

    # expose other things
    components componentsFiltered;

  # export the set used to build shell alongside the main derivation
  # downstream devs can extend/override the shell as needed
  # holonix-shell provides canonical dev shell for generic work
  main = derivation-safe-holonix-shell;
}
