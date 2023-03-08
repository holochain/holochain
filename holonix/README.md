# Holonix

this implementation of holonix uses the flake- and crane-based nix expressions.

coarse compatibility with the previous holonix interface is provided so that
all previously working nix expressions targetting the holonix repository still
work.

more advanced customization features are now possible via the flake's native
input override feature.

as an exmaple, here is a _flake.nix_ that references a custom branch.

```nix=
{
  description = "Template for Holochain app development";

  inputs = {
    nixpkgs.follows = "holochain/nixpkgs";

    holochain = {
      url = "github:holochain/holochain/pr_holonix_on_flakes";
      inputs.versions.url = "github:holochain/holochain/?dir=versions/0_1";
      inputs.versions.inputs.holochain.url = "github:holochain/holochain/holochain-0.1.3";
    };
  };

  outputs = inputs @ { ... }:
    inputs.holochain.inputs.flake-parts.lib.mkFlake
      {
        inherit inputs;
      }
      {
        systems = builtins.attrNames inputs.holochain.devShells;
        perSystem =
          { config
          , pkgs
          , system
          , ...
          }: {
            devShells.default = pkgs.mkShell {
              inputsFrom = [ inputs.holochain.devShells.${system}.holonix ];
              packages = with pkgs; [
                  # more packages go here
              ];
            };
          };
      };
}
```

this exmaple would translate to the following CLI invocatin

```shell=
nix develop \
  github:holochain/holochain#holonix \
  --override-input versions 'github:holochain/holochain/?dir=versions/0_1' \
  --override-input versions/holochain 'github:holochain/holochain/holochain-0.1.3'
```
