# Core Testing

This is a small guide for holochain core testing. This should not be used to test your hApp's code, but instead to test holochain's core code. To test hApp code, use [tryorama](https://github.com/holo-host/tryorama).

Holochain's unit tests for holochain core are located just beside the source code. You can open any rust source code file, and it will have at the end its own tests. Also you can find integration tests in the `test` folder in each crate. To know where you should place your tests, follow [Rust's conventions for testing](https://doc.rust-lang.org/book/ch11-03-test-organization.html).

There are tests at different levels of integration, so you can tailor your tests to whatever components or flows you want. For example, there are tests that only call functions for a workflow to validate its procedure, but there are also tests that boot up a conductor and call functions to it simulating a UI.

These tests will be run every time you post a new pull request in CircleCI.

## Requirements

Either of these:

- *(recommended)* Having `nix` installed ([instructions](https://nixos.org/download.html)).
- *(alternative)* Having rust and `cargo` installed and in the stable toolchain

## Running tests


### Using the same Nix derivations as CI

CI runs all Holochain tests via the `nix build` command by referencing the various packages.
As of 2023-03-02, we have the following test derivations:

- test-all
- test-static-all
- test-static-clippy
- test-static-doc
- test-static-fmt
- test-unit
- test-unit-all
- test-unit-tx5
- test-unit-wasm

The ones ending in *-all* are meta packages, combining all tests in the same category.

The following command builds the meta-package that incorporates all Holochain tests, and passes the current directory as the source that's to be tested:

```
nix build -L \
  --override-input holochain . \
  .#test-all
```

### Using test preconfigured impure test scripts

First of all, from the root folder, run `nix develop .#coreDev`.

The _coreDev_ developer shell provides impure test scripts that are automatically generated from the Nix derivations that we use for testing on CI.
They are prefixed with *script-* and you should be able to autocomplete them by typing _script-<TAB>_.

For example to run all tests from all the crates, run this from the root folder:

```bash
script-holochain-tests-all
```

Or use `script-holochain-tests-unit-all` if you don't want to run the static checks (cargo doc, cargo fmt and cargo clippy), but all of these this will be run on CI.

### Filtering tests

To run the tests for a specific package you can pass a filter to `nextest`

```shell
env NEXTEST_EXTRA_ARGS="-E 'package(hdk)'" nix build --impure --override-input holochain . .#test-unit
```

Or you can select a test by name

```shell
env NEXTEST_EXTRA_ARGS="-E 'test(paths_exists)'" nix build --impure --override-input holochain . .#test-unit
```

### Using `cargo test` manually

- To run only one test from your crate, run this command:

```bash
cd crates/holochain
cargo test [NAME_OF_THE_TEST_FUNCTION] --lib  --features 'slow_tests build_wasms' -- --nocapture
```

If the test is located in a crate other than `holochain`, `cd` into its folder instead.

## Adding a test

The documentation for the Holochain test harness is [here](https://docs.rs/holochain/latest/holochain/sweettest/index.html). Or you can run `cargo doc -p holochain --open` to view the documentation from your current branch. 

The simplest way to add a new test is to locate a similar test, copy and paste it, and modify the necessary instructions to test what you want. Don't forget to change its name. Note that these tests are for testing holochain core, and not the wasms themselves. To unit test a wasm, the test should be added to the wasm.

## Adding a custom zome to call from a test

The tests can access and call WASM-compiled zomes that are present in the `test_utils` folder. To add new custom zomes in that folder:

1. Add the zome's source code inside the `crates/test_utils/wasm/wasm_workspace` folder.
2. In the `crates/test_utils/wasm/src/lib.rs` file:
- Add the zome's name in TitleCase in the `TestWasm` enum.
- Add a match arm inside the `impl From<TestWasm> for ZomeName` implementation that points to the folder you have created.
- Add a new match arm inside the `impl From<TestWasm> for DnaWasm` implementation with the same path as all other arms but with the zome's name.
3. In the `members` property of the `crates/test_utils/wasm/wasm_worskpace/Cargo.toml` file, add the folder name for your zome.

## Examples

- [Call zome functions from your test](https://github.com/holochain/holochain/blob/develop/crates/holochain/src/core/ribosome/host_fn/commit_entry.rs#L234)
- [Boot up the conductor and call zome functions](https://github.com/holochain/holochain/blob/develop/crates/holochain/tests/ser_regression.rs)
