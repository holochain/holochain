{
  inputs =
    {
      holochain = {
        url = "github:holochain/holochain/holochain-0.2.6";
        flake = false;
      };

      lair = {
        url = "github:holochain/lair/lair_keystore-v0.4.2";
        flake = false;
      };

      # holochain_cli_launch
      launcher = {
        url = "github:holochain/launcher/holochain-0.2";
        flake = false;
      };

      # holochain_scaffolding_cli
      scaffolding = {
        url = "github:holochain/scaffolding/holochain-0.2";
        flake = false;
      };
    };

  outputs = { ... }: { };
}
