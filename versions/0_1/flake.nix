{
  inputs =
    {
      holochain = {
        url = "github:holochain/holochain/holochain-0.1.3";
        flake = false;
      };

      lair = {
        url = "github:holochain/lair/lair_keystore-v0.2.3";
        flake = false;
      };

      # holochain_cli_launch
      launcher = {
        url = "github:holochain/launcher/holochain_cli_launch-0.0.9";
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
