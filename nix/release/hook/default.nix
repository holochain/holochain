{ pkgs, config }:
{
  buildInputs = [ ]

    ++ (pkgs.callPackage ./version {
    config = config;
  }).buildInputs
  ;
}
