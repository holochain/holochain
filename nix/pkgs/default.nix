let
  flake = import ../compat.nix;

  ci = import ./ci.nix;
  core = import ./core.nix;
  happ = {
    inherit (flake.${builtins.currentSystem})
      happ-holochain
      happ-hc
      ;
  };

  all = {
    inherit
      ci
      core
      happ
      ;
  };

in builtins.mapAttrs (k: v:
  builtins.removeAttrs v [ "override" "overrideDerivation" ]
) all
