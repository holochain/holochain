{ git, gitAndTools, cacert }: {
  buildInputs = [
    git
    gitAndTools.git-hub
    cacert
    # need the haskellPackages version for darwin support
    # broken
    # pkgs.haskellPackages.github-release
  ];
}
