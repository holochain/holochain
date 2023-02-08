{...} @ args: let
  deprecatedArgs = [
    "config"
    "holochain-nixpkgs"
    "rustc"
    "holochainVersion"
    "include"
    "isIncludedFn"
    "includeHolochainBinaries"
  ];
  unsupportedArgs = ["holochainVersionId"];
  ignoredArgs = ["inNixShell"];

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

  flake = import ../nix/compat.nix;
  devShellsSystem = flake.devShells.${builtins.currentSystem};

  fn = {devShellId ? "holonix"}:
    devShellsSystem.${devShellId}.overrideAttrs (attrs:
      attrs
      // {
        passthru =
          (attrs.passthru or {})
          // {
            internal = {
              inherit flake;
              inherit devShellsSystem;
            };
          };
      });
in (fn filteredArgs)
