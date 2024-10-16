{
  inputs =
    {
      holochain = {
        url = "github:holochain/holochain/holochain-0.5.0-dev.0";
        flake = false;
      };

      lair = {
        url = "github:holochain/lair/lair_keystore-v0.5.2";
        flake = false;
      };

      # holochain_cli_launch
      launcher = {
        url = "github:holochain/hc-launch/holochain-weekly";
        flake = false;
      };

      # holochain_scaffolding_cli
      scaffolding = {
        url = "github:holochain/scaffolding/8a6d1dab0f1668c2781a46d93a5ad638fcf25598";
        flake = false;
      };
    };

  outputs = { ... }: { };
}
