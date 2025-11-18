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
      legacyPackages.sdkVersion = "12.0";
    };
}
