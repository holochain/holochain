{
  inputs =
    {
      holochain = {
        url = "github:holochain/holochain/holochain-0.5.0-dev.21";
        flake = false;
      };

      lair = {
        url = "github:holochain/lair/lair_keystore-v0.5.3";
        flake = false;
      };

      # holochain_cli_launch
      launcher = {
        url = "github:holochain/hc-launch/holochain-weekly";
        flake = false;
      };

      # holochain_scaffolding_cli
      scaffolding = {
        url = "github:holochain/scaffolding/holochain-weekly";
        flake = false;
      };
    };

  outputs = { ... }: {
    rustVersion = "1.87.0";
  };
}
