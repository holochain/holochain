{
  description = "Template for Holochain app development that uses a specific versions set";

  inputs = {
    holochain-flake.url = "github:holochain/holochain/experiment_flake_lock_mangling";

    holochain-versions.url = "github:holochain/holochain/experiment_flake_lock_mangling?dir=versions/0_1";

    holochain-flake.inputs.holochain.follows = "holochain-versions/holochain";
    holochain-flake.inputs.lair.follows = "holochain-versions/lair";
    holochain-flake.inputs.launcher.follows = "holochain-versions/launcher";
    holochain-flake.inputs.scaffolding.follows = "holochain-versions/scaffolding";

    nixpkgs.follows = "holochain-flake/nixpkgs";
    flake-parts.follows = "holochain-flake/flake-parts";
  };

  outputs = inputs @ { ... }:
    inputs.flake-parts.lib.mkFlake { inherit inputs; }
      {
        flake.inputs = inputs;
        systems = builtins.attrNames inputs.holochain-flake.devShells;

        perSystem =
          { inputs'
          , config
          , pkgs
          , system
          , ...
          }: {

            devShells.default = pkgs.mkShell {
              inputsFrom = [ inputs'.holochain-flake.devShells.holonix ];
              packages = with pkgs; [
                # more packages go here
              ];
            };
          };
      };
}
