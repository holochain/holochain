{ self, lib, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }: {
    options.coreScripts = lib.mkOption {
      type = lib.types.lazyAttrsOf lib.types.raw;
    };
    config.coreScripts = {
      inherit (self'.packages)
        hcTest
        hcStandardTests
        hcStandardTestsNextest
        hcWasmTests
        hcReleaseAutomationTest
        hcReleaseAutomationTestRepo
        hcStaticChecks
        hcMergeTest
        hcReleaseTest
        hcSpeedTest
        hcFlakyTest
        hcDoctor
        hcBench
        hcFmtAll
        hcBenchGithub
        hcRegenReadmes
        hcRegenNixExpressions
        ;
    }
    // (lib.optionalAttrs pkgs.stdenv.isLinux {
      inherit (self'.packages)
        hcCoverageTest
        ;
    });
  };
}
