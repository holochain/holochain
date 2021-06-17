# holochain_cli

Provides the `hc` binary, a helpful CLI tool for working with Holochain.

```shell
$ hc -h

holochain_cli 0.1.0
Holochain CLI

Work with DNA and hApp bundle files, set up sandbox environments for testing and development purposes, make direct admin
calls to running conductors, and more.

USAGE:
    hc <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    app        Work with hApp bundles
    dna        Work with DNA bundles
    help       Prints this message or the help of the given subcommand(s)
    sandbox    Work with sandboxed environments for testing and development
```

## Docs

Each top-level subcommand is implemented as a separate crate. See:

- [holochain_cli_bundle](https://github.com/holochain/holochain/tree/develop/crates/hc_bundle) for more info on the `hc app` and `hc dna` commands
- [holochain_cli_sandbox](https://github.com/holochain/holochain/tree/develop/crates/hc_sandbox) for more info on the `hc sandbox` command

## Installation

### Requirements

- [Rust](https://rustup.rs/)
- [Holochain](https://github.com/holochain/holochain) binary on the path

### Building

From github:

```shell
cargo install holochain_cli --git https://github.com/holochain/holochain
```

From the holochain repo:

```shell
cargo install --path crates/hc
```
