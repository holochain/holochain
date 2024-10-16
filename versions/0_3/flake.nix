{
  inputs =
    {
      holochain = {
        url = "github:holochain/holochain/holochain-0.3.3";
        flake = false;
      };

      lair = {
        url = "github:holochain/lair/lair_keystore-v0.4.5";
        flake = false;
      };

      # holochain_cli_launch
      launcher = {
        url = "github:holochain/hc-launch/holochain-0.3";
        flake = false;
      };

      # holochain_scaffolding_cli
      scaffolding = {
        url = "github:holochain/scaffolding/holochain-0.3";
        flake = false;
      };
    };

  outputs = { ... }: { };
}
