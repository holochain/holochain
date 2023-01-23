{ stdenv, lib, linuxPackages }: {
  buildInputs = [ ] ++ lib.optionals stdenv.isLinux [ linuxPackages.perf ];
}
