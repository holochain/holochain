# Holochain

[![Project](https://img.shields.io/badge/project-holochain-blue.svg?style=flat-square)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/chat-forum%2eholochain%2enet-blue.svg?style=flat-square)](https://forum.holochain.org)
[![Chat](https://img.shields.io/badge/chat-chat%2eholochain%2enet-blue.svg?style=flat-square)](https://chat.holochain.org)

[![Twitter Follow](https://img.shields.io/twitter/follow/holochain.svg?style=social&label=Follow)](https://twitter.com/holochain)
License: [![License: CAL 1.0](https://img.shields.io/badge/License-CAL%201.0-blue.svg)](https://github.com/holochain/cryptographic-autonomy-license)

This repository contains the core Holochain libraries and binaries.

This is the most recent and well maintained version of Holochain with a refactored state model (you may see references to it as Holochain RSM).

## Code Status

This code is in alpha. It is not for production use. The code is guaranteed NOT secure.

We will be frequently and heavily restructuring code APIs and data chains until Beta.

**We are currently only supporting Linux at this time**. You may or may not be able to successfully build and run Holochain on macOS. You definitely won't be able to on Windows (unless you are using WSL, but even that is untested). We will definitely be rolling out support for these OSes in the future, but in the meantime please use Linux for development!

## Making the Holochain binaries available in your shell
There are a number of contexts and purposes you might be running in which yield different ways to access binaries.


### Using nix-shell on a local clone
Assuming you have [installed the nix shell](https://nixos.wiki/wiki/Nix_Installation_Guide):

```
nix-shell --argstr flavor happDev
```

This nix-shell flavor installs wrapper binaries for `holochain` and `dna-util` that will automatically compile and run the binaries.  This is very useful if you are tracking changes in the holochain repo because when you check out a new rev, running holochain will compile automatically to the version at that rev.

### Building with cargo if you already have rust installed:
```
cargo install --path crates/holochain
cargo install --path crates/hc
```
## Usage

``` bash
$ holochain --help
USAGE:
    holochain [FLAGS] [OPTIONS]

FLAGS:
    -h, --help           Prints help information
    -i, --interactive    Receive helpful prompts to create missing files and directories,
                             useful when running a conductor for the first time
    -V, --version        Prints version information

OPTIONS:
    -c, --config-path <config-path>
            Path to a YAML file containing conductor configuration
```

Running `holochain` requires a config file.  You can generate one in the default configuration file locations using interactive mode:

``` bash
$ holochain -i
There is no conductor config YAML file at the path specified (/home/eric/.config/holochain/conductor-config.yml)
Would you like to create a default config file at this location? [Y/n]
Y
Conductor config written.
There is no database environment set at the path specified (/home/eric/.local/share/holochain/databases)
Would you like to create one now? [Y/n]
Y
LMDB environment created.
Conductor ready.
```

As well as creating the config file this process also instantiates the initial LMDB database environment.   If you provide a config file on first run with just the `-c` flag `holochain` will also initialize the environment even if not in interactive mode.

## Development Environment

Assuming you have [installed the nix shell](https://nixos.wiki/wiki/Nix_Installation_Guide):

```
git clone git@github.com:holochain/holochain.git
cd holochain
nix-shell
hc-merge-test
```

This will compile holochain and run all the tests.

If you get an error while running `nix-shell` about ngrok having an unfree license, you can fix that by running,

```
mkdir -p ~/.config/nixpkgs/
echo "{ allowUnfree = true; }" >> ~/.config/nixpkgs/config.nix
```

We have an all-in-one development environment including (among other things):

- The correct version and sane environment variables of cargo/rust
- Node for working with tryorama
- Scaffolding, build and deployment scripts
- Prebuilt binaries of core for various operating systems (soon)
- Shared libs such as libsodium

It is called [Holonix](https://github.com/holochain/holonix) and you should use it.

It has plenty of documentation and functionality and can be used across Windows, Mac, and Linux.
(Although Holochain itself currently only supports Linux.)
It is based on the development tools provided by [NixOS](http://nixos.org/).

It is suitable for use in hackathons and 'serious' development for a long-term,
production grade development team.

If you want to maintain your own development environment then we can only offer
rough advice, because anything we say today could be out of date tomorrow:

- Use a recent stable version of rust
- Use node 12x+ for clientside work
- Install any relevant shared libs like libsodium
- Write your own scaffolding, build and development tools
- Plan for dependency management as we ship new binaries

## Application Developer

[Read the wasm API docs](./crates/hdk/README.md)

Build the hdk docs:
```bash
cargo doc --manifest-path=crates/hdk/Cargo.toml --open
```

## Core Developer

Build the holochain docs:
```bash
cargo doc --manifest-path=crates/holochain/Cargo.toml --open
```

## Contribute
Holochain is an open source project.  We welcome all sorts of participation and are actively working on increasing surface area to accept it.  Please see our [contributing guidelines](/CONTRIBUTING.md) for our general practices and protocols on participating in the community, as well as specific expectations around things like code formatting, testing practices, continuous integration, etc.

* Connect with us on our [forum](https://forum.holochain.org)

## License
 [![License: CAL 1.0](https://img.shields.io/badge/License-CAL%201.0-blue.svg)](https://github.com/holochain/cryptographic-autonomy-license)

Copyright (C) 2019 - 2020, Holochain Foundation

This program is free software: you can redistribute it and/or modify it under the terms of the license
provided in the LICENSE file (CAL-1.0).  This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
PURPOSE.
