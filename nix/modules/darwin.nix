{ self
, lib
, ...
} @ flake: {
  perSystem =
    { config
    , system
    , pkgs
    , ...
    }: {
      legacyPackages.apple_sdk' = lib.attrsets.optionalAttrs pkgs.stdenv.isDarwin
        (
          # aarch64 only uses 11.0 and x86_64 mixes them
          if system == "x86_64-darwin"
          then pkgs.darwin.apple_sdk_10_12
          else pkgs.darwin.apple_sdk_11_0
        );
    };
}
