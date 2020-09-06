# Testing

## Requirements

Having `nix-shell` installed ([instructions](https://nixos.org/download.html)).

## Unit tests

Unit tests for holochain are located just beside the source code. You can open any rust source code file, and it will have at the end its own tests. This tests are executed in a sandboxed environment, not bringing up the whole holochain conductor. For this, we'll use integration tests with tryorama (TODO: is this right?)

These tests will be run every time you post a new pull request in CircleCI.

### Running tests

- To run all tests in from all the crates, run this from the root folder:

```bash
nix-shell
hc-merge-test
```

- To run only one test from your crate, run this command:

```bash
cargo test --manifest-path=crates/holochain/Cargo.toml --features slow_tests -- --nocapture [NAME_OF_THE_TEST_FUNCTION]
```

If the test is located in a crate other than `holochain`, point to that crate in the `--manifest-path` argument.

### Adding a unit test

The simplest way to add a new unit test is to locate a similar test, copy and paste it, and modify the necessary instructions to test what you want. Don't forget to change its name.

### Adding a custom zome to call from a unit test

The unit tests can access and call WASM-compiled zomes that are present in the `test_utils` folder. To add new custom zomes in that folder:

1. Add the zome's source code inside the `crates/test_utils/wasm/wasm_workspace` folder.
2. In the `crates/test_utils/wasm/src/lib.rs` file:
- Add the zome's name in TitleCase in the `TestWasm` enum.
- Add a match arm inside the `impl From<TestWasm> for ZomeName` implementation that points to the folder you have created.
- Add a new match arm inside the `impl From<TestWasm> for DnaWasm` implementation with the same path as all other arms but with the zome's name.

### Calling zome functions from a unit test

To call zome functions from your unit test:

```rust
let mut host_access = fixt!(ZomeCallHostAccess);
host_access.workspace = workspace_lock.clone();

// get the result of a commit entry
let output: CommitEntryOutput =
    crate::call_test_ribosome!(host_access, TestWasm::CommitEntry, "commit_entry", ());
```

Full example in `crates/holochain/src/core/ribosome/host_fn/commit_entry.rs`.

## Integration tests (with tryorama)

TODO