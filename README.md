[![Project](https://img.shields.io/badge/Project-Holochain-blue.svg?style=flat-square)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/Forum-forum%2eholochain%2enet-blue.svg?style=flat-square)](https://forum.holochain.org)
[![License: CAL 1.0](https://img.shields.io/badge/License-CAL%201.0-blue.svg)](https://github.com/holochain/cryptographic-autonomy-license)
![Test](https://github.com/holochain/holochain-client-js/actions/workflows/test.yml/badge.svg?branch=main)
[![Twitter Follow](https://img.shields.io/twitter/follow/holochain.svg?style=social&label=Follow)](https://twitter.com/holochain)

# Holochain

Holochain is an open-source framework to develop peer-to-peer applications with high levels of security, reliability, and performance.
This repository contains the Holochain core libraries.

## Code Status

This code is in alpha. It is not for production use. We will be frequently and heavily restructuring code APIs and data chains until Beta.

**We are supporting Linux and macOS at this time**. You may or may not be able to build and run Holochain on Windows.
We will be supporting it in the future.

## Holochain Development and Usage

### Running Holochain applications (hApps)

Holochain apps can be installed and run using the [Holochain Launcher](https://github.com/holochain/launcher). Refer to the Github repository
for instructions on how to obtain and use it.

### Developing Holochain applications (hApps)

Looking to write a hApp yourself? You need to set up your development environment and then can scaffold your hApp. Refer to the
[Holochain Dev Tools Guide](https://developer.holochain.org/install) to set up your environment, and see
[how hApps are scaffolded](https://developer.holochain.org/happ-setup/#scaffolding-a-new-happ) for creating a hApp template.

Furthermore there's a tutorial on how to [build a hApp](https://github.com/holochain/happ-build-tutorial) as well as
[how to call it from a client](https://github.com/holochain/happ-client-call-tutorial).

### Contributing to Holochain (this repository)

Holochain is an open source project. We welcome all sorts of participation and are actively working on increasing surface area to accept it. Please see our [contributing guidelines](/CONTRIBUTING.md) for our general practices and protocols on participating in the community, as well as specific expectations around things like code formatting, testing practices, continuous integration, etc.

> Connect with us on [Discord](https://discord.gg/MwPvM4Vffg) and [our forum](https://forum.holochain.org).

#### Development environment setup

The recommended way to set up your development environment is to use the [Holochain Dev Tools](https://developer.holochain.org/install), also called "Holonix".
Holonix is a specification of the necessary tools and libraries to write and build hApps, based on Nix. This guarantees a deterministic development
environment across machines and operating systems.

For a reference on how to set up your environment without Holonix, see the corresponding installation instructions at
[Install Holochain without Holonix](https://developer.holochain.org/install-without-holonix).

### Usage

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

Running `holochain` requires a config file. You can generate one in the default configuration file locations using interactive mode:

``` bash
$ holochain -i
There is no conductor config YAML file at the path specified (/home/eric/.config/holochain/conductor-config.yml)
Would you like to create a default config file at this location? [Y/n]
Y
Conductor config written.
There is no database set at the path specified (/home/eric/.local/share/holochain/databases)
Would you like to create one now? [Y/n]
Y
Database created.
Conductor ready.
```

As well as creating the config file this process also instantiates the database.   If you provide a config file on first run with just the `-c` flag `holochain` will also initialize the environment even if not in interactive mode.

## License

Copyright (C) 2019, Holochain Foundation

This program is free software: you can redistribute it and/or modify it under the terms of the license
provided in the LICENSE file (CAL-1.0). This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
PURPOSE.
