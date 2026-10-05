# Holochain Client - Rust

[![Project](https://img.shields.io/badge/project-holochain-blue.svg?style=flat-square)](http://holochain.org/)
[![Discord](https://img.shields.io/badge/Discord-blue.svg?style=flat-square)](https://discord.gg/k55DS5dmPH)
[![License: CAL 1.0](https://img.shields.io/badge/License-CAL%201.0-blue.svg)](https://github.com/holochain/cryptographic-autonomy-license)
[![Twitter Follow](https://img.shields.io/twitter/follow/holochain.svg?style=social&label=Follow)](https://twitter.com/holochain)
![Test](https://github.com/holochain/holochain-client-rust/actions/workflows/test.yml/badge.svg?branch=main)

Types and bindings to connect easily to a running Holochain conductor from Rust.

## Compatibility

**Rust client v0.7.x** is compatible with **Holochain v0.5.x**.

**Rust client v0.6.x** is compatible with **Holochain v0.4.x**.

**Rust client v0.5.x** is compatible with **Holochain v0.3.x**.

## Connection resilience

`AdminWebsocket` and `AppWebsocket` are single connections. When the conductor
restarts, they stop working and the caller reconnects.

`ReconnectingAdminWebsocket` and `ReconnectingAppWebsocket` repair themselves.
Use `connect` when the conductor should already be running and a failure is
worth reporting, and `connect_with_retry` when you are waiting for one to
start; the latter never gives up, so bound it with `tokio::time::timeout` if
you need it to.

Admin connections can use a hostname (for example, `"localhost:30000"`);
the hostname is resolved again for each reconnect attempt. A concrete socket
address or address list is reused unchanged. Custom `ToSocketAddrs`
implementations should be converted to an address list before connecting.

Set `ReconnectingAppWebsocket::builder(...).websocket_config(config)` to use
a custom `Arc<WebsocketConfig>` for the initial app connection and every
reconnect. This controls app transport limits and request timeouts; it does
not change the admin connection's configuration.

Requests made while a connection is down return
`ConductorApiError::Disconnected`. Before the client notices a dead connection,
a request may instead return the underlying websocket error. Neither is
retried automatically: a failed zome call may already have written to the
source chain, and re-signing it would mint a new nonce and risk a second write.

Signals emitted while a client is disconnected are lost — Holochain has no
signal replay. A `SignalStream` therefore reports `SignalEvent::Interrupted`
when it resumes, so state derived from signals can be re-read.

## Running the tests

``` bash
./build-fixture.sh
cargo test --release
```

## Contribute
Holochain is an open source project. We welcome all sorts of participation and are actively working on increasing surface area to accept it. Please see our [contributing guidelines](/CONTRIBUTING.md) for our general practices and protocols on participating in the community, as well as specific expectations around things like code formatting, testing practices, continuous integration, etc.

## License
 [![License: CAL 1.0](https://img.shields.io/badge/License-CAL%201.0-blue.svg)](https://github.com/holochain/cryptographic-autonomy-license)

Copyright (C) 2020-2023, Holochain Foundation

This program is free software: you can redistribute it and/or modify it under the terms of the license
provided in the LICENSE file (CAL-1.0).  This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
PURPOSE.
