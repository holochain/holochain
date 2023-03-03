{ self, lib, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }: {
    options.lib = lib.mkOption { type = lib.types.raw; };
    config.lib.mkMetaPkg = name: dependencies: pkgs.stdenv.mkDerivation {
      inherit name;
      dontUnpack = true;
      installPhase = ''
        mkdir $out
      '' + builtins.concatStringsSep "\n" (builtins.map (pkg: "${pkgs.coreutils}/bin/ln -sf ${pkg} $out/${pkg.name or pkg.pname}")
        dependencies
      );

      passthru = { inherit dependencies; };
    };


  };
  flake = { };
}
