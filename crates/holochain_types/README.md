# crate

[![Project](https://img.shields.io/badge/project-holochain-blue.svg?style=flat-square)](http://holochain.org/)
[![Discord](https://img.shields.io/badge/Discord-blue.svg?style=flat-square)](https://discord.gg/k55DS5dmPH)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)

Common types used by other Holochain crates.

This crate is a complement to the [holochain_zome_types crate](https://crates.io/crates/holochain_zome_types), which contains only the essential types which are used in Holochain DNA code. This crate expands on those types to include all types which Holochain itself depends on.

**It is not recommended to depend on this crate from your zomes**, as it is not guaranteed to compile for the `wasm32-unknown-unknown` target, and even if it does, it will pull in many needless dependencies, bloating your Wasm. If there is a type from `holochain_types` that you absolutely need in your DNA, please [open an issue in the holochain repo](https://github.com/holochain/holochain/issues) explaining why, and we can consider pulling that type into `holochain_zome_types`.

## Contribute
Holochain is an open source project.  We welcome all sorts of participation and are actively working on increasing surface area to accept it.  Please see our [contributing guidelines](/CONTRIBUTING.md) for our general practices and protocols on participating in the community, as well as specific expectations around things like code formatting, testing practices, continuous integration, etc.

## License
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)

Copyright (C) 2019 - 2024, Holochain Foundation

This program is free software: you can redistribute it and/or modify it under the terms of the license
provided in the LICENSE file (Apache 2.0).  This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
PURPOSE.
