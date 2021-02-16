{ writeShellScriptBin
, holonixPath
}:

{
  ciSetupNixConf = writeShellScriptBin "hc-ci-setup-nix-conf.sh" ''
    ${holonixPath}/ci/setup-hydra-cache.sh
  '';
}
