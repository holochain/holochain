{
  inputs =
    {
      holochain = {
        url = "github:holochain/holochain";
        flake = false;
      };

      lair = {
        url = "github:holochain/lair";
        flake = false;
      };

      # holochain_cli_launch
      launcher = {
        url = "github:holochain/launcher";
        flake = false;
      };

      # holochain_scaffolding_cli
      scaffolding = {
        url = "github:holochain/scaffolding/pr_holonix_on_flakes_compat";
        flake = false;
      };
    };

  outputs = { ... }: { };
}
