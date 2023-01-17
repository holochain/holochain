{
  lib,
  pkgs,
  devShells,
  holochainSrc,
}:
devShells.coreDev.overrideAttrs (attrs: {
  buildInputs = attrs.nativeBuildInputs ++ ([
    pkgs.niv
    pkgs.cargo-readme
    (import (holochainSrc + /crates/release-automation/default.nix) {
      inherit pkgs;
    })
  ]);
})
