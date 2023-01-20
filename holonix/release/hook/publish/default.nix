{ pkgs, config }: {
  buildInputs = [ ]

    ++ (pkgs.callPackage ./crates-io { config = config; }).buildInputs

  ;
}
