{ writeShellScriptBin
, holonixPath
}:

{
  ciSetupNixConf = writeShellScriptBin "hc-ci-setup-nix-conf.sh" ''
    ${holonixPath}/ci/setup-hydra-cache.sh
    ${holonixPath}/ci/cachix.sh setup
  '';

  ciCachixPush = writeShellScriptBin "hc-ci-cachix-push.sh" ''
    ${holonixPath}/ci/cachix.sh push
  '';
}
