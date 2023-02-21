{ system ? builtins.currentSystem, ... } @ args:
let
  deprecatedArgs = [
    "config"
    "holochain-nixpkgs"
    "rustc"
    "holochainVersion"
    "include"
    "isIncludedFn"
    "includeHolochainBinaries"
  ];
  unsupportedArgs = [ "holochainVersionId" ];
  ignoredArgs = [ "inNixShell" ];

  deprecatedArgs' =
    builtins.filter
      (arg:
        if builtins.hasAttr arg args
        then
          builtins.trace
            "[WARNING] the argument '${arg}' has been deprecated and rendered ineffective."
            true
        else false)
      deprecatedArgs;
  unsupportedArgs' =
    builtins.filter
      (arg:
        if builtins.hasAttr arg args
        then
          builtins.trace
            "[WARNING] the argument '${arg}' is currently unimplemented. it will either be implemented or deprecated in the near future."
            true
        else false)
      unsupportedArgs;
  filteredArgs =
    builtins.removeAttrs args
      (unsupportedArgs' ++ deprecatedArgs' ++ ignoredArgs);

  flake-compat = import ../nix/compat.nix;
  devShellsSystem = flake-compat.devShells.${system};

  fn = { devShellId ? "holonix" }:
    devShellsSystem.${devShellId}.overrideAttrs (attrs:
      attrs
      // {
        passthru =
          (attrs.passthru or { })
          // {
            pkgs = flake-compat.legacyPackages.${system};
            main = devShellsSystem.${devShellId};

            internal = {
              inherit flake-compat;
            };
          };
      });
in
(fn filteredArgs)
