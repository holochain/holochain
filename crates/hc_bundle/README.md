# hc_bundle

[![Project](https://img.shields.io/badge/project-holochain-blue.svg?style=flat-square)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/chat-forum%2eholochain%2enet-blue.svg?style=flat-square)](https://forum.holochain.org)
[![Chat](https://img.shields.io/badge/chat-chat%2eholochain%2enet-blue.svg?style=flat-square)](https://chat.holochain.org)

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)

Current version: 0.0.1

Utilities to create DNA and hApp bundle files from a source working directory, and vice-versa.
This crate defines two separate subcommands for the `hc` CLI tool, one for each type of bundle.
Both subcommands are very similar and have identical interfaces.

This crate also defines standalone binaries for each subcommand, `hc-dna` and `hc-app`.

Usage instructions from the `-h` flag:

```sh
$ hc dna -h

hc-dna 0.1.0
Work with DNA bundles

USAGE:
    hc dna <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    help      Prints this message or the help of the given subcommand(s)
    init      Create a new, empty Holochain DNA bundle working directory
    pack      Pack the contents of a directory into a `.dna` bundle file
    unpack    Unpack the parts of `.dna` file out into a directory
```

`hc app -h` is very similar.

## Contribute
Holochain is an open source project.  We welcome all sorts of participation and are actively working on increasing surface area to accept it.  Please see our [contributing guidelines](/CONTRIBUTING.md) for our general practices and protocols on participating in the community, as well as specific expectations around things like code formatting, testing practices, continuous integration, etc.

* Connect with us on our [forum](https://forum.holochain.org)

## License
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)

Copyright (C) 2019 - 2021, Holochain Foundation

This program is free software: you can redistribute it and/or modify it under the terms of the license
provided in the LICENSE file (Apache 2.0).  This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
PURPOSE.
