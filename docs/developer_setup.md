## Setting up a Holochain development environment

### Pre-requisites

Development is supported and regularly done on Ubuntu Linux and MacOS. Other operating systems may work, but you may
need extra steps that are not in this guide. 

Holochain runs on Windows and it may be possible to develop on Windows, but none of the Holochain maintainers use 
Windows so again, there may be extra steps that are not in this guide. If you do develop on Windows, please consider
using the Windows Subsystem for Linux (WSL) with Ubuntu, which is expected to work.

You will need:
- Git installed, either [directly](https://git-scm.com/downloads) or via your package manager.
- Rust installed via [rustup](https://www.rust-lang.org/tools/install). The toolchain version is selected automatically
  from `rust-toolchain.toml`.
- [cargo-nextest](https://nexte.st/), the configured test runner. The Make targets below use it, so it is required to
  run the tests the way CI does.
- [Make](https://www.gnu.org/software/make/), which may already be on your system or can be installed via your package
  manager. The project's build, test, and lint steps are encoded as Make targets that bundle the exact features and
  flags CI uses. Reproducing those by hand is fiddly, so Make is the recommended way to run them.

### Finding your way around

The Holochain repository contains many Rust crates and some important files:
- /rust-toolchain.toml: This file specifies the Rust toolchain used by the project. When you start running `cargo`
  commands, it will automatically use the Rust version specified here.
- /Makefile: The make targets that lint, build and test the project. They encode the exact checks and test runs that CI
  performs, and are the recommended way to build, test, and lint locally.
- /crates/test_utils/wasm/wasm_workspace: This is a separate Cargo workspace that contains the test WASMs that are used
  in Holochain's tests. These need to be built before running the tests, which is explained later in the guide.
- /crates/hc: The `hc` command line tool which is used to interact with a running Holochain instance.
- /crates/holochain: The main Holochain library/binary crate. This is what we call the "conductor" and is where you'll
  find the majority of the functional tests.

### Build and verify your environment

Before changing any code, it's a good idea to check that your environment can build Holochain. Rather than compiling the
whole workspace, build the binaries that the `hc sandbox` tests rely on — `holochain`, `hc`, and `hc_sandbox`. This is
enough to confirm your toolchain is set up, and it produces the tools you'll use for manual testing later:

```shell
cargo build --manifest-path crates/holochain/Cargo.toml --locked
cargo build --manifest-path crates/hc/Cargo.toml --locked
cargo build --manifest-path crates/hc_sandbox/Cargo.toml --locked
```

The first build will take some time, as it downloads and compiles all the dependencies. Subsequent builds are quicker
because Cargo caches the compiled crates. If these succeed, your environment is ready.

If you have problems here you can [get in touch](https://github.com/holochain/holochain/blob/develop/CONTRIBUTING.md#coordination)
or open an issue.

### Running tests

The default, full test command — and the one CI runs — is the Make target for the default Wasmer backend (cranelift) and
iroh transport:

```shell
make test-workspace-wasmer-sys-cranelift
```

This builds the test wasms and runs the whole workspace with the exact features CI uses. It takes a long time, so you
don't need to run it for every change; CI will run it on your pull request.

While you're iterating, run only the tests for the crate(s) you're changing. For example, if you're changing
`holochain_p2p`:

```shell
cd crates/holochain_p2p
cargo nextest run
```

Before you finish, run the `holochain` crate's tests. This crate holds the bulk of the integration tests and needs the
test wasms built:

```shell
cd crates/holochain
cargo nextest run --features build_wasms
```

### Testing other Wasmer backends

Holochain executes zome Wasm through a Wasmer backend, chosen by a set of mutually exclusive features — exactly one must
be enabled. CI exercises each backend, and there is a Make target for each:

- `wasmer-sys-cranelift` — the default compiler backend:
  ```shell
  make test-workspace-wasmer-sys-cranelift
  ```
- `wasmer-sys-llvm` — the LLVM compiler backend, which requires a compatible LLVM toolchain to be installed:
  ```shell
  make test-workspace-wasmer-sys-llvm
  ```
- `wasmer-wasmi` — the Wasm interpreter backend:
  ```shell
  make test-workspace-wasmer-wasmi
  ```

The `Makefile` also has variants that turn on the unstable features (`*-unstable`) and the tx5 transport
(`*-transport_tx5`); use those targets if you need to reproduce one of those CI runs.

### Static checks before submitting a PR

Before opening a pull request, run the same static checks that CI enforces:

```shell
make static-all
```

This runs a formatting check (`cargo fmt --check`), a TOML formatting check, Clippy over the default and unstable feature
sets, and a documentation build with warnings denied. Compiler and Clippy warnings are not allowed in shared code; see
[Compiler warnings](https://github.com/holochain/holochain/blob/develop/CONTRIBUTING.md#compiler-warnings).

To fix issues rather than only check for them:

```shell
cargo fmt --all              # format Rust code
./scripts/format-toml.sh     # format TOML files (add --check to only check)
```

`scripts/format-toml.sh` runs `taplo` through `nix-shell`, so it needs Nix installed. If you don't use Nix, `make
static-toml` (check) and `make toml-fix` (fix) do the same job with a Cargo-installed `taplo`.

### Verifying changes and reproducing issues

If you are able to create a [sweettest](https://github.com/holochain/holochain/tree/develop/crates/holochain/src/sweettest) 
test case that reproduces an issue, then that is a great way to make sure the issue stays fixed. There are many tests 
written with this harness, so take a look at what's already there as a guide for writing new tests.

Otherwise, you can test your changes or try to reproduce an issue manually using the `hc sandbox`. This tool is used to 
launch a `holochain` instance (conductor) that has been built locally. You can find the documentation for this tool [here](https://github.com/holochain/holochain/blob/develop/crates/hc_sandbox/README.md).
You'll need the CLI tools built for this (see [Build and verify your environment](#build-and-verify-your-environment) above); the next section explains how to run or install them.

### Using the CLI tools for manual testing

The build step above produced the `holochain`, `hc`, and `hc_sandbox` binaries. Some tests — for example those in
`crates/hc_sandbox` — run these real binaries and check that they interact correctly, and you'll also want them to drive
a conductor manually.

They can be run from the `target/debug` directory. For example, to run the `hc` tool:

```shell
./target/debug/hc --help
```

If you find it easier to have these tools in your `PATH`, then you can install them instead of just building them:

```shell
cargo install --path crates/holochain --locked
cargo install --path crates/hc --locked
cargo install --path crates/hc_sandbox --locked
```

Note that this requires that you have set up your Rust environment to have the Cargo install directory in your `PATH`.
Please see the [`cargo-install`](https://doc.rust-lang.org/cargo/commands/cargo-install.html).
