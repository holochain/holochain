## Setting up a Holochain development environment

### Pre-requisites

You will need one of the following

- *recommended* - Having `nix` installed following these [instructions](https://nixos.org/download.html).
- *alternative* - Having Rust installed via [rustup](https://www.rust-lang.org/tools/install) and configured to use the stable toolchain.

### Build holochain

If using Nix, start by opening a developer shell using `nix develop .#coreDev` at the root of the repository. This will take
some time you run it.

Now you can build the project with `cargo build`.

This is a good check that your environment is ready to use. If you have problems here you can [get in touch](https://github.com/holochain/holochain/blob/develop/CONTRIBUTING.md#coordination) or open an issue.

### Run the tests

There is a [testing guide](https://github.com/holochain/holochain/blob/develop/docs/core_testing.md) which will get you started running
the tests the same way the CI does.

### Build CLI tools

Other documentation will assume you have access to CLI tools provided by this repository, so it's a good idea to build these now.
You should rebuild these as needed when making changes or pulling changed code from your upstream branch, e.g. `develop`.

```shell
cargo install --path crates/holochain --locked
cargo install --path crates/hc --locked
cargo install --path crates/hc_sandbox --locked
cargo install --path crates/hc_signal_srv --locked
```

### Verifying changes and reproducing issues

If you are able to create a sweettest test case that reproduces an issue then that is a great way to make sure the issue stays fixed.

Otherwise, you can test your changes or try to reproduce an issue manually using the `hc sandbox`. This tool is used to launch a `holochain` instance (conductor) that
has been built locally. You can find the documentation for this tool [here](https://github.com/holochain/holochain/blob/develop/crates/hc_sandbox/README.md).
