{ stdenv, lib, darwin }:
let
  # https://stackoverflow.com/questions/51161225/how-can-i-make-macos-frameworks-available-to-clang-in-a-nix-environment
  frameworks = darwin.apple_sdk.frameworks;
  ld-flags =
    "-F${frameworks.CoreFoundation}/Library/Frameworks -framework CoreFoundation ";

in
lib.attrsets.optionalAttrs stdenv.isDarwin {
  buildInputs =
    [ frameworks.Security frameworks.CoreFoundation frameworks.CoreServices ];

  shellHook = ''
    LD_FLAGS="$LDFLAGS${ld-flags}";
  '';
}
