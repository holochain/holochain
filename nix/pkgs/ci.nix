{
  inherit ((import ../compat.nix).packages.${builtins.currentSystem})
    ciSetupNixConf
    ciCachixPush
    ;
}
