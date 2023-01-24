{ pkgs, config }:
let
  # derive some release specific config
  release-config = config // {
    release = config.release // {
      # release tag name
      tag = "v${config.release.version.current}";

      # name of the branch used to facilitate release
      branch = "release-${config.release.version.current}";

      # canonical git remote name as per `git remote -v`
      upstream = "origin";
    };
  };
in {
  buildInputs = [ ]

    ++ (pkgs.callPackage ./branch { config = release-config; }).buildInputs

    ++ (pkgs.callPackage ./changelog { config = release-config; }).buildInputs

    ++ (pkgs.callPackage ./cut { config = release-config; }).buildInputs

    ++ (pkgs.callPackage ./github { config = release-config; }).buildInputs

    ++ (pkgs.callPackage ./hook { config = release-config; }).buildInputs

    ++ (pkgs.callPackage ./push { config = release-config; }).buildInputs;

  config = release-config;
}
