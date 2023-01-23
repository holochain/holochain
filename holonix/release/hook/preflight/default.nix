{ pkgs, config }: {
  buildInputs = [ ]

    ++ (pkgs.callPackage ./manual { config = config; }).buildInputs

  ;
}
