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
    # the latest crate2nix is currently broken on darwin
    ++ (lib.optionals pkgs.stdenv.isLinux [
      crate2nix
    ])
    ++ (lib.optionals pkgs.stdenv.isDarwin 
      (with pkgs.darwin; [
        Security
        IOKit
        apple_sdk_11_0.frameworks.CoreFoundation
      ])
    )
    ;
}
