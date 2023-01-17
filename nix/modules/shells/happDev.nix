{
  lib,
  pkgs,
  hcMkShell,
  devShells,
  ciSetupNixConf,
  ciCachixPush,
  happ-holochain,
  happ-hc,
}:
hcMkShell {
  inputsFrom = [
    (builtins.removeAttrs devShells.coreDev [ "shellHook" ])
  ];
  nativeBuildInputs =
    [
      happ-holochain
      happ-hc
    ]
    ++ (with pkgs; [
    sqlcipher
    binaryen
    gdb
  ])
  ;
}
