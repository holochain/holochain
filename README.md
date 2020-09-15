# Holochain

[![Project](https://img.shields.io/badge/project-holochain-blue.svg?style=flat-square)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/chat-forum%2eholochain%2enet-blue.svg?style=flat-square)](https://forum.holochain.org)
[![Chat](https://img.shields.io/badge/chat-chat%2eholochain%2enet-blue.svg?style=flat-square)](https://chat.holochain.org)

[![Twitter Follow](https://img.shields.io/twitter/follow/holochain.svg?style=social&label=Follow)](https://twitter.com/holochain)
License: [![License: CAL 1.0](https://img.shields.io/badge/License-CAL%201.0-blue.svg)](https://github.com/holochain/cryptographic-autonomy-license)

This repository contains most of the core Holochain libraries and binaries.

**Code Status:** This code is in alpha. It is not for production use. The code is guaranteed NOT secure. Also, We will likely be restructuring code APIs and data chains until Beta.

## Overview

// TODO: remove or add the correct link?
[Holochain Implementation Architectural Overview](./docs/TODO.md)

## Development Environment

Assuming you have [installed the nix shell](https://nixos.wiki/wiki/Nix_Installation_Guide):

```
git clone git@github.com:holochain/holochain.git
cd holochain
nix-shell
hc-merge-test
```

This will compile holochain and run all the tests.

We have an all-in-one development environment including (among other things):

- The correct version and sane environment variables of cargo/rust
- Node for working with tryorama
- Scaffolding, build and deployment scripts
- Prebuilt binaries of core for various operating systems (soon)
- Shared libs such as libsodium

It is called [Holonix](https://github.com/holochain/holonix) and you should use it.

It has plenty of documentation and functionality and can be used across Windows, Mac, and Linux.
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
