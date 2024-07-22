# Holonix

this implementation of holonix uses the flake- and crane-based nix expressions.

more advanced customization features are now possible via the flake's native
input override feature.

## recommended versioning specification in a consumer's flake.nix

#### use the "versions" flake as a separate input in their flake.nix and configuring the holochain flake to follow the versions flake, as in:

```nix
inputs = {
  holochain-versions.url = "github:holochain/holochain?dir=versions/0_2";

  holochain-flake.url = "github:holochain/holochain";
  holochain-flake.inputs.versions.follows = "holochain-versions";
};
```

#### override single components either via the holochain versions flake:

```nix
inputs = {
  holochain-versions.url = "github:holochain/holochain?dir=versions/0_2";
  holochain-versions.inputs.holochain.url = "github:holochain/holochain/holochain-0.2.6";

  holochain-flake.url = "github:holochain/holochain";
  holochain-flake.inputs.versions.follows = "holochain-versions";
};
```

or via their the toplevel component input:

```nix
inputs = {
  holochain-versions.url = "github:holochain/holochain?dir=versions/0_2";

  holochain-flake.url = "github:holochain/holochain";
  holochain-flake.inputs.versions.follows = "holochain-versions";

  holochain-flake.inputs.holochain.url = "github:holochain/holochain/holochain-0.2.6";
};
```

please see the following examples to learn more about common and more specific use cases:

* [specifying custom component versions](examples/custom_versions/flake.nix)

## Customizing the holochain binary build parameters

The top-level flake output `packages.holochain` and `devShells.holonix` are customisable by means of [nixpkgs.lib.makeOverridable](https://nixos.org/manual/nixpkgs/stable/#sec-lib-makeOverridable).

### Example: pass `--features chc` to holochain's `cargo build` command

This means that you can pass e.g. `holochain.override { cargoExtraArgs = " --feature chc"; }` or any other desirable attribute to override the attributes that are passed to [craneLib.buildPackage](https://crane.dev/API.html#cranelibbuildpackage).

In a devShell based on holonix, this can be achieved by specifying `holonix.override { holochainOverrides = { cargoExtraArgs = "--features chc"; }}`.
Please see [this flake](examples/custom_holochain_feature/flake.nix) for a complete example.