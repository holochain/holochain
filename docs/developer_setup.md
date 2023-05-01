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

### Verifying changes and reproducing issues

If you are able to create a [sweettest](https://github.com/holochain/holochain/tree/develop/crates/holochain/src/sweettest) test case that reproduces an issue then that is a great way to make sure the issue stays fixed.
There are many tests written with this harness, so take a look at what's already there as a guide for writing new tests.

Otherwise, you can test your changes or try to reproduce an issue manually using the `hc sandbox`. This tool is used to launch a `holochain` instance (conductor) that
has been built locally. You can find the documentation for this tool [here](https://github.com/holochain/holochain/blob/develop/crates/hc_sandbox/README.md).
You'll want to read the next section for instructions to build all the tools you might want to use with the sandbox.

### Build CLI tools for manual testing

If you need to interact with a running conductor or start one for testing then there are CLI tools provided for that.
You should rebuild these as needed when making changes or pulling changed code from your upstream branch, e.g. `develop`.

```shell
cargo install --path crates/holochain --locked
cargo install --path crates/hc --locked
cargo install --path crates/hc_sandbox --locked
cargo install --path crates/hc_signal_srv --locked
```
