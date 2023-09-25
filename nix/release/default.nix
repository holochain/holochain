{ pkgs, config }:
{
  buildInputs = [ ]

    ++ (pkgs.callPackage ./hook {
    config = config;
  }).buildInputs
  ;
}
