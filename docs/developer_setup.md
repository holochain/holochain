## Setting up a Holochain development environment

### Pre-requisites

Development is supported and regularly done on Ubuntu Linux and MacOS. Other operating systems may work, but you may
need extra steps that are not in this guide. 

Holochain runs on Windows and it may be possible to develop on Windows, but none of the Holochain maintainers use 
Windows so again, there may be extra steps that are not in this guide. If you do develop on Windows, please consider
using the Windows Subsystem for Linux (WSL) with Ubuntu, which is expected to work.

You will need:
- Git installed, either [directly](https://git-scm.com/downloads) or via your package manager.
- Rust installed via [rustup](https://www.rust-lang.org/tools/install).
- [cargo-nextest](https://nexte.st/) which is recommended to run Holochain's tests. Though you can use `cargo test` if you prefer.

You may find more tools useful:
- [Make](https://www.gnu.org/software/make/), which may already be on your system, or you can install it via your package manager.

### Finding your way around

The Holochain repository contains many Rust crates and some important files:
- /rust-toolchain.toml: This file specifies the Rust toolchain used by the project. When you start running `cargo`
  commands, it will automatically use the Rust version specified here.
- /Makefile: A list of make targets that can be used to lint, build and test the project. Using this is optional, but it
  can be useful to run the same checks and tests that the CI runs.
- /crates/test_utils/wasm/wasm_workspace: This is a separate Cargo workspace that contains the test WASMs that are used
  in Holochain's tests. These need to be built before running the tests, which is explained later in the guide.
- /crates/hc: The `hc` command line tool which is used to interact with a running Holochain instance.
- /crates/holochain: The main Holochain library/binary crate. This is what we call the "conductor" and is where you'll
  find the majority of the functional tests.

### Build and test Holochain

Before changing any code, it's a good idea to check your environment is ready to use. You can do this by building the 
project and running the tests.

To build the project, you can run:
```shell
cargo build
```

This will take some time, as it will download the necessary dependencies and compile the code. It also has to build all
those dependencies. This will be quicker the next time you run it, as Cargo will cache the compiled crates.

Now, run the Holochain tests:

```shell
cd crates/holochain
cargo nextest run --features build_wasms
```

If you have problems here you can [get in touch](https://github.com/holochain/holochain/blob/develop/CONTRIBUTING.md#coordination) 
or open an issue.

### What tests to run

Note that you can run all the tests in the repository by running the same command as above but from the root of the 
repository. However, this will take a really long time and is generally not recommended.

Instead, please run the tests for any crates that you are changing. For example, if you are changing `holochain_p2p`
then you should run the tests for that crate:

```shell
cd crates/holochain_p2p
cargo nextest run
```

Once you are done making changes to a crate, or crates, and those tests are passing, then you should run the tests for
Holochain as above.

Of course, we welcome people running all the tests in the repository, but running fewer tests saves some time and CI
will check that everything is passing before merging your changes. If you do run into problems with this approach,
see the next section for how to run the tests like CI does.

### Run the tests like CI does

This is optional, but if you make changes and your changes are not passing the tests locally but not on CI, then this 
is a useful way to check what's going on.

You would have to check which CI checks are failing, but as an example, you might run the tests with the default Wasmer
runtime, from the root of the repository:

```shell
make test-workspace-wasmer_sys
```

### Verifying changes and reproducing issues

If you are able to create a [sweettest](https://github.com/holochain/holochain/tree/develop/crates/holochain/src/sweettest) 
test case that reproduces an issue, then that is a great way to make sure the issue stays fixed. There are many tests 
written with this harness, so take a look at what's already there as a guide for writing new tests.

Otherwise, you can test your changes or try to reproduce an issue manually using the `hc sandbox`. This tool is used to 
launch a `holochain` instance (conductor) that has been built locally. You can find the documentation for this tool [here](https://github.com/holochain/holochain/blob/develop/crates/hc_sandbox/README.md).
You'll want to read the next section for instructions to build all the tools you might want to use with the sandbox.

### Build CLI tools for manual testing

Some of the tests run in the Holochain repository, run real binaries and check that they interact correctly. For example,
the tests in `crates/hc_sandbox` do this. For these tests to work, you need to have built the CLI tools that are part
of this repository.

Otherwise, if you just want to use the CLI tools to interact with a running conductor or start one for testing then 
you would also want to build these CLI tools.

```shell
cargo build --manifest-path crates/holochain/Cargo.toml --locked
cargo build --manifest-path crates/hc/Cargo.toml --locked
cargo build --manifest-path crates/hc_sandbox/Cargo.toml --locked
```

The tools can then be run from the `target/debug` directory. For example, to run the `hc` tool, you can run:

```shell
./target/debug/hc --help
```

If you find it easier to have these tools in your `PATH`, then you can install these tools instead of just building them:

```shell
cargo install --path crates/holochain --locked
cargo install --path crates/hc --locked
cargo install --path crates/hc_sandbox --locked
```

Note that this requires that you have set up your Rust environment to have the Cargo install directory in your `PATH`.
Please see the [`cargo-install`](https://doc.rust-lang.org/cargo/commands/cargo-install.html).
