# holochain_sqlite

[![Project](https://img.shields.io/badge/project-holochain-blue.svg?style=flat-square)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/chat-forum%2eholochain%2enet-blue.svg?style=flat-square)](https://forum.holochain.org)
[![Chat](https://img.shields.io/badge/chat-chat%2eholochain%2enet-blue.svg?style=flat-square)](https://chat.holochain.org)

[![Twitter Follow](https://img.shields.io/twitter/follow/holochain.svg?style=social&label=Follow)](https://twitter.com/holochain)
License: [![License: CAL 1.0](https://img.shields.io/badge/License-CAL%201.0-blue.svg)](https://github.com/holochain/cryptographic-autonomy-license)

Current version: 0.0.1

## Building blocks for persisted Holochain state

### History

Originally, this crate was written to target LMDB. After it had already stabilized, we completely refactored it to naively use SQLite as a key-value store, instead. This is in preparation for using more intelligently and fully, using carefully chosen indexes and queries. However, for now, the structure of this crate can only be understood in the context of this major recent refactor.

### Backend: SQLite

Persistence is not generalized for different backends: it is targeted specifically for SQLite. In the future, if we have to change backends (again), or if we have to support something like IndexedDb, we will generalize the interface just enough to cover both.

### Buffered Stores

The unit of persisted Holochain state is the [BufferedStore]. This interface groups three things together:

- A reference to a SQLite database
- A reference to a read-only transaction (shared by other stores)
- A "scratch space", which is a HashMap into which write operations get staged (the buffer)

The purpose of the scratch space is to prevent the need for opening a read-write transaction, of which there can be only one at a time. With the buffer of the scratch space, store references can live for a more leisurely length of time, accumulating changes, and then the buffer can be flushed all at once in a short-lived read-write transaction.

Note that a BufferedStore includes a reference to a read-only transaction, which means that the store acts as a snapshot of the persisted data at the moment it was constructed. Changes to the underlying persistence will not be seen by this BufferedStore.

See the [buffer] crate for implementations.

#### Strong typing

All BufferedStores are strongly typed. All keys and values must be de/serializable, and so de/serialization happens automatically when getting and putting items into stores. As a consequence, the source chain CAS is split into two separate DBs: one for Entries, and one for Headers.

### Workspaces

The intention is that Holochain code never deals with individual data stores directly, individually. BufferedStores are always grouped into a Workspace, which is a collection of stores that's been put together for a specific purpose. A workspace may choose to provide open access to the underlying stores, or it may protect them behind a purpose-built interface.

The stores in a Workspace are all provided a common read-only transaction, so their snapshots are all consistent with each other at the moment in time the workspace was constructed. The workspace provides its own interface for interacting with the stores. Once changes have been accumulated in the BufferedStores, the Workspace itself can be committed, which uses a fresh read-write transaction to flush the changes from each store and commit them to disk. Committing consumes the Workspace.

Workspaces themselves are implemented in the `holochain` crate

### Building blocks

The `holochain_sqlite` crate provides three buffered KV store abstractions as well as a simple CAS abstraction:

- [KvBuf]: a normal KV store
- [KvIntBuf]: a KV store where keys must be integers (this was significant when using LMDB, but not any more)
- [KvvBuf]: a KV store with multiple values per key, with per-key iteration
- [CasBuf]: a [KvBuf] which enforces that keys must be the "address" of the values (content)

The `holochain` crate composes these building blocks together to build more purpose-specific BufferedStore implementations

See [this hackmd](https://holo.hackmd.io/@holochain/SkuVLpqEL) for a diagram explaining the relationships between these building blocks and the higher abstractions

## Contribute
Holochain is an open source project.  We welcome all sorts of participation and are actively working on increasing surface area to accept it.  Please see our [contributing guidelines](/CONTRIBUTING.md) for our general practices and protocols on participating in the community, as well as specific expectations around things like code formatting, testing practices, continuous integration, etc.

* Connect with us on our [forum](https://forum.holochain.org)

## License
 [![License: CAL 1.0](https://img.shields.io/badge/License-CAL-1.0-blue.svg)](https://github.com/holochain/cryptographic-autonomy-license)

Copyright (C) 2019 - 2022, Holochain Foundation

This program is free software: you can redistribute it and/or modify it under the terms of the license
provided in the LICENSE file (CAL-1.0).  This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
PURPOSE.
