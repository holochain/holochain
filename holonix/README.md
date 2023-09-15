# Holonix

this implementation of holonix uses the flake- and crane-based nix expressions.

more advanced customization features are now possible via the flake's native
input override feature.

## recommended versioning specification in a consumer's flake.nix

#### use the "versions" flake as a separate input in their flake.nix and configuring the holochain flake to follow the versions flake, as in:

```nix
inputs = {
  holochain-versions.url = "github:holochain/holochain?dir=versions/0_1";

  holochain-flake.url = "github:holochain/holochain";
  holochain-flake.inputs.versions.follows = "holochain-versions";
};
```

#### override single components either via the holochain versions flake:

```nix
inputs = {
  holochain-versions.url = "github:holochain/holochain?dir=versions/0_1"; holochain-versions.inputs.holochain.url = "github:holochain/holochain/holochain-0.1.5-beta-rc.0";

  holochain-flake.url = "github:holochain/holochain";
  holochain-flake.inputs.versions.follows = "holochain-versions";
};
```

or via their the toplevel component input:

```nix
inputs = {
  holochain-versions.url = "github:holochain/holochain?dir=versions/0_1";

  holochain-flake.url = "github:holochain/holochain";
  holochain-flake.inputs.versions.follows = "holochain-versions";

  holochain-flake.inputs.holochain.url = "github:holochain/holochain/holochain-0.1.5-beta-rc.0";
};
```

please see the following examples to learn more about common and more specific use cases:

* [specifying custom component versions](examples/custom_versions/flake.nix)
