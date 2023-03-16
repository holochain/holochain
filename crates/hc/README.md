# holochain_cli

Provides the `hc` binary, a helpful CLI tool for working with Holochain.

```shell
$ hc -h
holochain_cli 0.1.3
Holochain CLI

Work with DNA, hApp and web-hApp bundle files, set up sandbox environments for testing and development purposes, make
direct admin calls to running conductors, and more.

EXTENSIONS:
    hc launch	  Run "hc launch --help" to see its help
    hc scaffold	  Run "hc scaffold --help" to see its help

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
    web-app    Work with Web-hApp bundles
```

## Docs

Some top-level subcommands are implemented as separate crates. See:

- [holochain_cli_bundle](https://github.com/holochain/holochain/tree/develop/crates/hc_bundle) for more info on the `hc app` and `hc dna` commands
- [holochain_cli_sandbox](https://github.com/holochain/holochain/tree/develop/crates/hc_sandbox) for more info on the `hc sandbox` command

This tool also supports 'extensions', commands whose names start with `hc-`, which are automatically made available as their own subcommands. See:

- [holochain/scaffolding](https://github.com/holochain/scaffolding) for more info on the `hc scaffold` extension
- [hc_launch in holochain/launcher](https://github.com/holochain/launcher/tree/main/crates/hc_launch) for more info on the `hc launch` extension

## Installation

### Quick install

Follow the [quick start guide](https://developer.holochain.org/quick-start/) on the Holochain Developer Portal to get set up with all the Holochain development tools, including the `hc` CLI and official extensions.

### Build from source

#### Requirements

- [Rust](https://rustup.rs/)
- [Holochain](https://github.com/holochain/holochain) binary on the path

#### Building

From github:

```shell
cargo install holochain_cli --git https://github.com/holochain/holochain
```

From the holochain repo:

```shell
cargo install --path crates/hc
```
