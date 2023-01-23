# Example: Custom Holochain

This example `default.nix` imports a file called `holochain_version.nix` which contains the value for the `holochainVersion` argument.
This value has been separated into a file to make it easier to programmatically generate it.
You can do generate this file with following command:

```shell
nix-shell https://github.com/holochain/holochain-nixpkgs/archive/develop.tar.gz \
  --pure \
  --arg flavors '["release"]' \
  --run '
    update-holochain-versions \
      --git-src=revision:holochain-0.0.129 \
      --lair-version-req='~0.0' \
      --output-file=holochain_version.nix
  '
```

`holochain-0.1.129` can be replaced with any commit hash or tag from the [Holochain repo](https://github.com/holochain/holochain), and `~0.0` can be replaced with any [SemVer specification](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html) for [lair_keystore](https://crates.io/crates/lair_keystore)

