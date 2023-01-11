{ self, lib, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }: {
    # Definitions like this are entirely equivalent to the ones
    # you may have directly in flake.nix.
    packages = let
      mkHolochainBinaryScript = crate: pkgs.writeShellScriptBin (builtins.replaceStrings ["_"] ["-"] crate) ''
        exec ${self'.packages.hcRunCrate}/bin/hc-run-crate ${crate} $@
      '';
    in {
      ciSetupNixConf = pkgs.writeShellScriptBin "hc-ci-setup-nix-conf.sh" ''
        ${self.srcCleaned}/ci/setup-hydra-cache.sh
        ${self.srcCleaned}/ci/cachix.sh setup
      '';

      ciCachixPush = pkgs.writeShellScriptBin "hc-ci-cachix-push.sh" ''
        ${self.srcCleaned}/ci/cachix.sh push
      '';

      hcReleaseAutomation = pkgs.writeShellScriptBin "hc-ra" ''
        exec ${self'.packages.hcRunCrate}/bin/hc-run-crate "release-automation" $@
      '';

      happ-holochain = mkHolochainBinaryScript "holochain";
      happ-hc = mkHolochainBinaryScript "hc";
    };
  };
}
