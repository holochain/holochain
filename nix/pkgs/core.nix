{
  inherit ((import ../compat.nix).packages.${builtins.currentSystem})
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
