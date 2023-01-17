{
  lib,
  pkgs,
  hcMkShell,
  devShells,
  ciSetupNixConf,
  ciCachixPush,
}:
hcMkShell {
  inputsFrom = [
    (builtins.removeAttrs devShells.coreDev [ "shellHook" ])
  ];
  nativeBuildInputs = [
    ciSetupNixConf
    ciCachixPush
  ];
}
