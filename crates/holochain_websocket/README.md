# holochain_websocket

Holochain utilities for websocket serving and connecting.

 To establish an outgoing connection, use [`connect`]
which will return a tuple
([`WebsocketSender`], [`WebsocketReceiver`])

To open a listening socket, use [`WebsocketListener::bind`]
which will give you a [`WebsocketListener`]
which is an async Stream whose items resolve to that same tuple (
[WebsocketSender](struct.WebsocketSender.html),
[WebsocketReceiver](struct.WebsocketReceiver.html)
).

If you want to be able to shutdown the stream use [`WebsocketListener::bind_with_handle`]
which will give you a tuple ([`ListenerHandle`], [`ListenerStream`]).
You can use [`ListenerHandle::close`] to close immediately or
[`ListenerHandle::close_on`] to close on a future completing.

## Example

```rust
use holochain_serialized_bytes::prelude::*;
use holochain_websocket::*;

use std::convert::TryInto;
use tokio_stream::StreamExt;
use url2::prelude::*;

#[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug)]
struct TestMessage(pub String);

// Create a new server listening for connections
let mut server = WebsocketListener::bind(
    url2!("ws://127.0.0.1:0"),
    std::sync::Arc::new(WebsocketConfig::default()),
)
.await
.unwrap();

// Get the address of the server
let binding = server.local_addr().clone();

tokio::task::spawn(async move {
    // Handle new connections
    while let Some(Ok((_send, mut recv))) = server.next().await {
        tokio::task::spawn(async move {
            // Receive a message and echo it back
            if let Some((msg, resp)) = recv.next().await {
                // Deserialize the message
                let msg: TestMessage = msg.try_into().unwrap();
                // If this message is a request then we can respond
                if resp.is_request() {
                    let msg = TestMessage(format!("echo: {}", msg.0));
                    resp.respond(msg.try_into().unwrap()).await.unwrap();
                }
            }
        });
    }
});

// Connect the client to the server
let (mut send, _recv) = connect(binding, std::sync::Arc::new(WebsocketConfig::default()))
    .await
    .unwrap();

let msg = TestMessage("test".to_string());
// Make a request and get the echoed response
let rsp: TestMessage = send.request(msg).await.unwrap();

assert_eq!("echo: test", &rsp.0,);
```

## Contribute
Holochain is an open source project.  We welcome all sorts of participation and are actively working on increasing surface area to accept it.  Please see our [contributing guidelines](/CONTRIBUTING.md) for our general practices and protocols on participating in the community, as well as specific expectations around things like code formatting, testing practices, continuous integration, etc.

* Connect with us on our [forum](https://forum.holochain.org)

## License
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)

Copyright (C) 2019 - 2023, Holochain Foundation

License: Apache-2.0
