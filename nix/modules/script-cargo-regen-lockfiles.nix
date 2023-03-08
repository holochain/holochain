{ self, lib, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }:
    let

      script-cargo-regen-lockfiles =
        pkgs.writeShellScriptBin "script-cargo-regen-lockfiles" ''
          cargo fetch --locked
          cargo generate-lockfile --offline --manifest-path=crates/test_utils/wasm/wasm_workspace/Cargo.toml
          cargo generate-lockfile --offline
          cargo generate-lockfile --offline --manifest-path=crates/test_utils/wasm/wasm_workspace/Cargo.toml
        '';

    in
    {
      packages = {
        inherit script-cargo-regen-lockfiles;
      };
    };
}
