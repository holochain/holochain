{
  inputs =
    {
      holochain = {
        url = "github:holochain/holochain/holochain-0.4.0-dev.11";
        flake = false;
      };

      lair = {
        url = "github:holochain/lair/lair_keystore-v0.4.5";
        flake = false;
      };

      # holochain_cli_launch
      launcher = {
        url = "github:holochain/launcher/holochain-weekly";
        flake = false;
      };

      # holochain_scaffolding_cli
      scaffolding = {
        url = "github:holochain/scaffolding/holochain-weekly";
        flake = false;
      };
    };

  outputs = { ... }: { };
}
