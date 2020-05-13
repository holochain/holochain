# dna_util

[![Project](https://img.shields.io/badge/project-holochain-blue.svg?style=flat-square)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/chat-forum%2eholochain%2enet-blue.svg?style=flat-square)](https://forum.holochain.org)
[![Chat](https://img.shields.io/badge/chat-chat%2eholochain%2enet-blue.svg?style=flat-square)](https://chat.holochain.org)

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)

Current version: 0.0.1

A utility to create a DNA file from a source working directory, and vice-versa

This utility expects a working directory of the following structure:
test-dna.dna.workdir/
├── dna.json
├── test-zome-1.wasm
└── test-zome-2.wasm

``` bash
$ dna_util --help

    dna_util 0.0.1
    Holochain DnaFile Utility.

    USAGE:
dna_util [OPTIONS]

    FLAGS:
-h, --help
    Prints help information

    -V, --version
    Prints version information


    OPTIONS:
-c, --compile <compile>
    Compile a Dna Working Directory into a DnaFile.

    (`dna_util -c my-dna.dna_work_dir` creates file `my-dna.dna.gz`)
    -e, --extract <extract>
    Extract a DnaFile into a Dna Working Directory.

    (`dna_util -e my-dna.dna.gz` creates dir `my-dna.dna_work_dir`)
``` bash

## Contribute
Holochain is an open source project.  We welcome all sorts of participation and are actively working on increasing surface area to accept it.  Please see our [contributing guidelines](/CONTRIBUTING.md) for our general practices and protocols on participating in the community, as well as specific expectations around things like code formatting, testing practices, continuous integration, etc.

* Connect with us on our [forum](https://forum.holochain.org)

## License
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)

Copyright (C) 2019-2020, Holochain Foundation

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at
