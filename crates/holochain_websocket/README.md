# holochain_websocket

[![Project](https://img.shields.io/badge/project-holochain-blue.svg?style=flat-square)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/chat-forum%2eholochain%2enet-blue.svg?style=flat-square)](https://forum.holochain.org)
[![Chat](https://img.shields.io/badge/chat-chat%2eholochain%2enet-blue.svg?style=flat-square)](https://chat.holochain.org)

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)

Current version: 0.0.1

Holochain utilities for websocket serving and connecting.

To establish an outgoing connection, use [websocket_connect](fn.websocket_connect.html)
which will return a tuple (
[WebsocketSender](struct.WebsocketSender.html),
[WebsocketReceiver](struct.WebsocketReceiver.html)
).

To open a listening socket, use [websocket_bind](fn.websocket_bind.html)
which will give you a [WebsocketListener](struct.WebsocketListener.html)
which is an async Stream whose items resolve to that same tuple (
[WebsocketSender](struct.WebsocketSender.html),
[WebsocketReceiver](struct.WebsocketReceiver.html)
).

## Example

```rust
#
use crate::*;

use url2::prelude::*;
use tokio::stream::StreamExt;
use std::convert::TryInto;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct TestMessage(pub String);
try_from_serialized_bytes!(TestMessage);

let mut server = websocket_bind(
    url2!("ws://127.0.0.1:0"),
    std::sync::Arc::new(WebsocketConfig::default()),
)
.await
.unwrap();

let binding = server.local_addr().clone();

tokio::task::spawn(async move {
    while let Some(maybe_con) = server.next().await {
        let (_send, mut recv) = maybe_con.unwrap();

        tokio::task::spawn(async move {
            if let Some(msg) = recv.next().await {
                if let WebsocketMessage::Request(data, respond) = msg {
                    let msg: TestMessage = data.try_into().unwrap();
                    let msg = TestMessage(
                        format!("echo: {}", msg.0),
                    );
                    respond(msg.try_into().unwrap()).await.unwrap();
                }
            }
        });
    }
});

let (mut send, _recv) = websocket_connect(
    binding,
    std::sync::Arc::new(WebsocketConfig::default()),
)
.await
.unwrap();

let msg = TestMessage("test".to_string());
let rsp: TestMessage = send.request(msg).await.unwrap();

assert_eq!(
    "echo: test",
    &rsp.0,
);
#
#
```

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
