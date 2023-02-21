 hdi

[![Project](https://img.shields.io/badge/project-holochain-blue.svg?style=flat-square)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/chat-forum%2eholochain%2enet-blue.svg?style=flat-square)](https://forum.holochain.org)
[![Chat](https://img.shields.io/badge/chat-chat%2eholochain%2enet-blue.svg?style=flat-square)](https://chat.holochain.org)

[![Twitter Follow](https://img.shields.io/twitter/follow/holochain.svg?style=social&label=Follow)](https://twitter.com/holochain)

[![Crate](https://img.shields.io/crates/v/hdi.svg)](https://crates.io/crates/hdi)
[![API Docs](https://docs.rs/hdi/badge.svg)](https://docs.rs/hdi)

<!-- cargo-rdme start -->

Holochain Deterministic Integrity (HDI) is Holochain's data model and integrity toolset for
writing zomes.

The logic of a Holochain DNA can be divided into two parts: integrity and coordination.
Integrity is the part of the hApp that defines the data types and validates data
manipulations. Coordination encompasses the domain logic and implements the functions
that manipulate data.

# Examples

An example of an integrity zome with data definition and data validation can be found in the
wasm workspace of the Holochain repository:
<https://github.com/holochain/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/integrity_zome/src/lib.rs>.

# Data definition

The DNA's data model is defined in integrity zomes. They comprise all data type definitions
as well as relationships between those types. Integrity zomes are purely definitions and do
not contain functions to manipulate the data. Therefore a hApp's data model is encapsulated
and completely independent of the domain logic, which is encoded in coordinator zomes.

The MVC (model, view, controller) design pattern can be used as an analogy. **The
application’s integrity zomes comprise its model layer** — everything that defines the shape
of the data. In practice, this means three things:
- entry type definitions
- link type definitions
- a validation callback that constrains the kinds of data that can validly be called entries
and links of those types (see also `Op`).

**The coordination zomes comprise the application's controller layer** — the code that actually
writes and retrieves data, handles countersigning sessions and sends and receives messages
between peers or between a cell and its UI. In other words, all the zome functions, `init`
functions, remote signal receivers, and scheduler callbacks will all live in coordinator zomes.

Advantages of this approach are:
* The DNA hash is constant as long as the integrity zomes remain the same. The peer network of
a DNA is tied to its hash. Changes to the DNA hash result in a new peer network. Changes to the
domain logic enclosed in coordinator zomes, however, do not affect the DNA hash. Hence the DNAs
and therefore hApps can be modified without creating a new peer network on every
deployment.
* Integrity zomes can be shared among DNAs. Any coordinator zome can import an integrity
zome's data types and implement functions for data manipulation. This composability of
integrity and coordinator zomes allows for a multitude of permutations with shared integrity
zomes, i. e. a shared data model.

# Data validation

The second fundamental part of integrity zomes is data validation. For every
operation
that is produced by an action, a
validation rule can be specified. Both data types and data values can be
validated.

All of these validation rules are declared in the `validate` callback. It
is executed for a new action by each validation authority.

There's a helper type called `FlatOp` available for easy
access to all link and entry variants when validating an operation. In many cases, this type can
be easier to work with than the bare `Op`. `FlatOp` contains the
same information as `Op` but with a flatter, more accessible data structure than `Op`'s deeply nested and concise structure.

```rust
match op.flattened()? {
    FlatOp::StoreEntry(OpEntry::CreateEntry { app_entry, .. }) => match app_entry {
        EntryTypes::A(_) => Ok(ValidateCallbackResult::Valid),
        EntryTypes::B(_) => Ok(ValidateCallbackResult::Invalid(
            "No Bs allowed in this app".to_string(),
        )),
    },
    FlatOp::RegisterCreateLink {
        base_address: _,
        target_address: _,
        tag: _,
        link_type,
    } => match link_type {
        LinkTypes::A => Ok(ValidateCallbackResult::Valid),
        LinkTypes::B => Ok(ValidateCallbackResult::Invalid(
            "No Bs allowed in this app".to_string(),
        )),
    },
    _ => Ok(ValidateCallbackResult::Valid),
};
```
See an example of the `validate` callback in an integrity zome in the WASM workspace:
<https://github.com/holochain/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/validate/src/integrity.rs>.
Many more validation examples can be browsed in that very workspace.

<!-- cargo-rdme end -->

## License
 [![License: CAL 1.0](https://img.shields.io/badge/License-CAL-1.0-blue.svg)](https://github.com/holochain/cryptographic-autonomy-license)

Copyright (C) 2019 - 2023, Holochain Foundation

This program is free software: you can redistribute it and/or modify it under the terms of the license
provided in the LICENSE file (CAL-1.0).  This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
PURPOSE.
