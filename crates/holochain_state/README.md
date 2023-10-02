
# holochain_state

[![Project](https://img.shields.io/badge/project-holochain-blue.svg?style=flat-square)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/chat-forum%2eholochain%2enet-blue.svg?style=flat-square)](https://forum.holochain.org)
[![Chat](https://img.shields.io/badge/chat-chat%2eholochain%2enet-blue.svg?style=flat-square)](https://chat.holochain.org)

[![Twitter Follow](https://img.shields.io/twitter/follow/holochain.svg?style=social&label=Follow)](https://twitter.com/holochain)

[![Crate](https://img.shields.io/crates/v/holochain_state.svg)](https://crates.io/crates/holochain_state)
[![API Docs](https://docs.rs/holochain_state/badge.svg)](https://docs.rs/holochain_state)

<!-- cargo-rdme start -->

The Holochain state crate provides helpers and abstractions for working
with the `holochain_sqlite` crate.

### Reads
The main abstraction for creating data read queries is the `Query` trait.
This can be implemented to make constructing complex queries easier.

The `source_chain` module provides the `SourceChain` type,
which is the abstraction for working with chains of actions.

The `host_fn_workspace` module provides abstractions for reading data during workflows.

### Writes
The `mutations` module is the complete set of functions
for writing data to sqlite in holochain.

### In-memory
The `scratch` module provides the `Scratch` type for
reading and writing data in memory that is not visible anywhere else.

The SourceChain type uses the Scratch for in-memory operations which
can be flushed to the database.

The Query trait allows combining arbitrary database SQL queries with
the scratch space so reads can union across the database and in-memory data.

<!-- cargo-rdme end -->

## License
 [![License: CAL 1.0](https://img.shields.io/badge/License-CAL-1.0-blue.svg)](https://github.com/holochain/cryptographic-autonomy-license)

Copyright (C) 2019 - 2023, Holochain Foundation

This program is free software: you can redistribute it and/or modify it under the terms of the license
provided in the LICENSE file (CAL-1.0).  This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
PURPOSE.
