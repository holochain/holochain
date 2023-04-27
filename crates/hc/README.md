# holochain_cli

Provides the `hc` binary, a helpful CLI tool for working with Holochain.

## Docs

This command gives you a suite of tools for developing, inspecting, executing, and testing your Holochain apps. Some top-level subcommands are implemented as separate crates, and others are separate binaries -- commands whose names start with `hc-` and are automatically made available as subcommands if they exist in your shell's path. Here is a list of all available subcommands:

- `hc dna`, `hc app`, and `hc web-app` scaffold, bundle, and unbundle DNAs, hApps and web hApps respectively. See [holochain_cli_bundle](https://github.com/holochain/holochain/tree/develop/crates/hc_bundle) for more info.
- `hc sandbox` creates and executes temporary or persistent conductor configurations for you to run test instances of your hApp with. See [holochain_cli_sandbox](https://github.com/holochain/holochain/tree/develop/crates/hc_sandbox) for more info.
- `hc signal-srv` runs a local WebRTC signal server for peers to establish connections with each other. See [holochain_cli_signal_srv](https://github.com/holochain/holochain/tree/develop/crates/hc_signal_srv) for more info.
- `hc scaffold` generates integrity, coordinator, UI, and test code for hApps using interactive prompts. See [holochain/scaffolding](https://github.com/holochain/scaffolding).
- `hc launch` runs sandboxed hApp instances with live-reloading UI windows. See [hc_launch in holochain/launcher](https://github.com/holochain/launcher/tree/main/crates/hc_launch) for more info.

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
