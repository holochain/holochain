{ ... }@args:

let
  deprecatedArgs = [
    "config"
    "holochain-nixpkgs"
    "rustc"
    "holochainVersion"
    "include"
    "includeHolochainBinaries"
    "isIncludedFn"
  ];

  ignoredArgs = [ "inNixShell" ];

  # TODO: filter args
  filteredArgs = args;
in
(

  {
    # either one listed in VERSIONS.md or "custom". when "custom" is set, `holochainVersion` needs to be specified
    holochainVersionId ? "main"
  , rustVersion ? { }
  , devShellId ? "holonix"
  }:

  let
    flake = (import ./compat.nix);
    devShellsSystem = flake.devShells.${builtins.currentSystem};
  in
  devShellsSystem."${devShellId}".overrideAttrs (attrs:
  attrs // {
    passthru = (attrs.passthru or { }) // {
      internal = {
        inherit flake;
        inherit devShellsSystem;
      };
    };
  })

) filteredArgs
