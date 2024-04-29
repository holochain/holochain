{ self, inputs, lib, ... }@flake: {
  perSystem = { config, self', inputs', system, pkgs, ... }:
    let
      format-toml = pkgs.writeShellScriptBin "format-toml" ''
        ${pkgs.taplo}/bin/taplo format ./*.toml
        ${pkgs.taplo}/bin/taplo format ./crates/**/*.toml
      '';

      format-toml-check = pkgs.writeShellScriptBin "format-toml" ''
        ${pkgs.taplo}/bin/taplo format ./*.toml --check
        ${pkgs.taplo}/bin/taplo format ./crates/**/*.toml --check
      '';
    in
    {
      packages = {
        inherit
          format-toml
          format-toml-check
          ;
      };
    };
}
