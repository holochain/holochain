# hc_bundle

[![Project](https://img.shields.io/badge/project-holochain-blue.svg?style=flat-square)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/chat-forum%2eholochain%2enet-blue.svg?style=flat-square)](https://forum.holochain.org)
[![Chat](https://img.shields.io/badge/chat-chat%2eholochain%2enet-blue.svg?style=flat-square)](https://chat.holochain.org)

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)

Utilities to create DNA, hApp, and web-hApp bundle files from a source working directory and manifest file, and vice-versa.
This crate defines three separate subcommands for the `hc` CLI tool, one for each type of bundle.
All subcommands are very similar and have nearly identical flags and options.

This crate also defines standalone binaries for each subcommand, `hc-dna`, `hc-app`, and `hc-web-app`.

Usage instructions from the `-h` flag:

```sh
$ hc dna -h
holochain_cli_bundle 0.1.3
Work with Holochain DNA bundles

USAGE:
    hc-dna <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    help      Prints this message or the help of the given subcommand(s)
    init      Create a new, empty Holochain DNA bundle working directory and create a new sample `dna.yaml` manifest
              inside. 
    pack      Pack into the `[name].dna` bundle according to the `dna.yaml` manifest, found inside the working
              directory. The `[name]` is taken from the `name` property of the manifest file
    unpack    Unpack parts of the `.dna` bundle file into a specific directory
```

`hc app` and `hc web-app` are very similar, only differing by the addition of a `--recursive` flag.
If used, this flag attempts to first pack all the assets to be included in the bundle being packed.
If it doesn't find the bundled DNA or hApp asset specified, it will by convention look for a
DNA or hApp manifest file in the same directory and attempt to pack it using the specified name.

## Contribute

Holochain is an open source project.  We welcome all sorts of participation and are actively working on increasing surface area to accept it.  Please see our [contributing guidelines](/CONTRIBUTING.md) for our general practices and protocols on participating in the community, as well as specific expectations around things like code formatting, testing practices, continuous integration, etc.

* Connect with us on our [forum](https://forum.holochain.org)

## License

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)

Copyright (C) 2019 - 2023, Holochain Foundation

This program is free software: you can redistribute it and/or modify it under the terms of the license
provided in the LICENSE file (Apache 2.0).  This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
PURPOSE.
