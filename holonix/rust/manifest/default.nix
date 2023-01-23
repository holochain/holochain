{ pkgs }: {
  buildInputs = [ ] ++ (pkgs.callPackage ./install { }).buildInputs
    ++ (pkgs.callPackage ./list-unpinned { }).buildInputs
    ++ (pkgs.callPackage ./set-ver { }).buildInputs
    ++ (pkgs.callPackage ./test-ver { }).buildInputs;
}
