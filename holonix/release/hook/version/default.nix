{ pkgs, config }: {
  buildInputs = [ ]

    ++ (pkgs.callPackage ./readme { config = config; }).buildInputs

    ++ (pkgs.callPackage ./rust { config = config; }).buildInputs;
}
