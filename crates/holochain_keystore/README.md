# crate

[![Project](https://img.shields.io/badge/project-holochain-blue.svg?style=flat-square)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/chat-forum%2eholochain%2enet-blue.svg?style=flat-square)](https://forum.holochain.org)
[![Chat](https://img.shields.io/badge/chat-chat%2eholochain%2enet-blue.svg?style=flat-square)](https://chat.holochain.org)

[![Twitter Follow](https://img.shields.io/twitter/follow/holochain.svg?style=social&label=Follow)](https://twitter.com/holochain)
License: [![License: CAL 1.0](https://img.shields.io/badge/License-CAL%201.0-blue.svg)](https://github.com/holochain/cryptographic-autonomy-license)

Current version: 0.0.1

A Keystore is a secure repository of private keys. KeystoreSender is a
reference to a Keystore. KeystoreSender allows async generation of keypairs,
and usage of those keypairs, reference by the public AgentPubKey.

## Example

```rust
use holo_hash::AgentPubKey;
use crate::*;
use holochain_serialized_bytes::prelude::*;

#[tokio::main(threaded_scheduler)]
async fn main() {
    tokio::task::spawn(async move {
        let _ = holochain_crypto::crypto_init_sodium();

        let keystore = test_keystore::spawn_test_keystore(vec![]).await.unwrap();
        let agent_pubkey = AgentPubKey::new_from_pure_entropy(&keystore).await.unwrap();

        #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
        struct MyData(Vec<u8>);

        let my_data_1 = MyData(b"signature test data 1".to_vec());

        let signature = agent_pubkey.sign(&keystore, &my_data_1).await.unwrap();

        assert!(agent_pubkey.verify_signature(&signature, &my_data_1).await.unwrap());
    }).await.unwrap();
}
```

## Contribute
Holochain is an open source project.  We welcome all sorts of participation and are actively working on increasing surface area to accept it.  Please see our [contributing guidelines](/CONTRIBUTING.md) for our general practices and protocols on participating in the community, as well as specific expectations around things like code formatting, testing practices, continuous integration, etc.

* Connect with us on our [forum](https://forum.holochain.org)

## License
 [![License: CAL 1.0](https://img.shields.io/badge/License-CAL-1.0-blue.svg)](https://github.com/holochain/cryptographic-autonomy-license)

Copyright (C) 2019 - 2021, Holochain Foundation

This program is free software: you can redistribute it and/or modify it under the terms of the license
provided in the LICENSE file (CAL-1.0).  This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
PURPOSE.
