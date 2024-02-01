## Setting up a Holochain development environment

### Pre-requisites

You will need one of the following

- *recommended* - Having `nix` installed following these [instructions](https://nixos.org/download.html).
- *alternative* - Having Rust installed via [rustup](https://www.rust-lang.org/tools/install) and configured to use the stable toolchain.

### Set up Cachix

Getting Holochain's build tools from source takes a long time to build, so we provide a cache so you can download binaries for your system.

If you installed Nix using Holochain's [getting started guide](https://developer.holochain.org/get-started/) then Cachix will have been set up for you. Otherwise you can set it up yourself by installing Cachix using their [install guide](https://docs.cachix.org/installation).

You can then run `cachix use holochain-ci`.

Now when you run `nix develop` or `nix build` commands, you should look out for the status line in your shell telling you that it's downloading pre-built binaries from `holochain-ci.cachix.org`.

### Build Holochain

If using Nix, start by opening a developer shell using `nix develop --override-input versions ./versions/weekly --override-input holochain . .#coreDev` at the root of the repository. This will take a while the first time you run it.

Now you can build the project with `cargo build`.

This is a good check that your environment is ready to use. If you have problems here you can [get in touch](https://github.com/holochain/holochain/blob/develop/CONTRIBUTING.md#coordination) or open an issue.

### Run the tests

There is a [testing guide](https://github.com/holochain/holochain/blob/develop/docs/core_testing.md) which will get you started running
the tests the same way the CI does.

### Verifying changes and reproducing issues

If you are able to create a [sweettest](https://github.com/holochain/holochain/tree/develop/crates/holochain/src/sweettest) test case that reproduces an issue, then that is a great way to make sure the issue stays fixed.
There are many tests written with this harness, so take a look at what's already there as a guide for writing new tests.

Otherwise, you can test your changes or try to reproduce an issue manually using the `hc sandbox`. This tool is used to launch a `holochain` instance (conductor) that
has been built locally. You can find the documentation for this tool [here](https://github.com/holochain/holochain/blob/develop/crates/hc_sandbox/README.md).
You'll want to read the next section for instructions to build all the tools you might want to use with the sandbox.

### Running Holonix from this repository

To get an environment which is similar to the Holonix environment you would use to develop a Holochain app, you can run

```shell
nix develop --override-input versions ./versions/weekly --override-input holochain . .#holonix
```

Take care to check what binaries are available in your environment because if you've run `cargo install --path crates/holochain` then that may appear first
in your `PATH`. Your binaries should appear in paths starting with `/nix/store`, and not include a `.cargo` directory. You can verify this yourself, e.g. via `which holochain`.

Once you have this shell open, it's a great place to test a happ with a custom Holochain version. Please be aware that changes made to the Holochain source code won't be automatically rebuilt into binaries. If you make changes then you'll need to `exit` and re-open the shell by runnihng the command above to create new binaries.

### Build CLI tools for manual testing

If you need to interact with a running conductor or start one for testing then there are CLI tools provided for that.
You should rebuild these as needed when making changes or pulling changed code from your upstream branch, e.g. `develop`.

```shell
cargo install --path crates/holochain --locked
cargo install --path crates/hc --locked
cargo install --path crates/hc_sandbox --locked
cargo install --path crates/hc_signal_srv --locked
```
