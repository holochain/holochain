{
  inputs =
    {
      holochain = {
        url = "github:holochain/holochain/holochain-0.4.1-rc.0";
        flake = false;
      };

      lair = {
        url = "github:holochain/lair/lair_keystore-v0.5.3";
        flake = false;
      };

      # holochain_cli_launch
      launcher = {
        url = "github:holochain/hc-launch/holochain-0.4";
        flake = false;
      };

      # holochain_scaffolding_cli
      scaffolding = {
        url = "github:holochain/scaffolding/holochain-0.4";
        flake = false;
      };
    };

  outputs = { ... }: {
    stub = true;
    rustVersion = "1.85.0";
  };
}
