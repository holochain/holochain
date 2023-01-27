{ self, lib, ... } @ flake: {
  # all possible parameters for perSystem: https://flake.parts/module-arguments.html#persystem-module-parameters
  perSystem = { config, self', inputs', pkgs, ... }: {
    devShells.default = pkgs.mkShell {
      packages = [ config.rust.rustHolochain self'.packages.holochain ];
    };

    devShells.coreDev =
      pkgs.mkShell {
        packages = [ config.rust.rustHolochain ];
      };
  };
}
