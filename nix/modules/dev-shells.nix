{ self, lib, ... } @ flake: {
  # all possible arguments for perSystem: https://flake.parts/module-arguments.html#persystem-module-parameters
  perSystem = { config, self', inputs', pkgs, ... }: {
    devShells.default = pkgs.mkShell {
      packages = [ self'.packages.holochain ];
    };
  };
}
