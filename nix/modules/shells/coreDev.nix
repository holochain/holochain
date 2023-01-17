{
  hcMkShell,
  lib,
  pkgs,
  crate2nix,
  rustc,
  cargo,
  coreScripts,
}:
hcMkShell {
  buildInputs =
    (builtins.attrValues (coreScripts))
    ++ (with pkgs;[
      cargo-nextest
      cargo-sweep
      gdb
      gh
      nixpkgs-fmt
      rustup
      sqlcipher
    ])
    ++ [
      cargo
      rustc
    ]
    # latest crate2nix broken on darwin
    ++ (lib.optionals pkgs.stdenv.isLinux [
      crate2nix
    ]);
}
