# holo_hash

[![Project](https://img.shields.io/badge/project-holochain-blue.svg?style=flat-square)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/chat-forum%2eholochain%2enet-blue.svg?style=flat-square)](https://forum.holochain.org)
[![Chat](https://img.shields.io/badge/chat-chat%2eholochain%2enet-blue.svg?style=flat-square)](https://chat.holochain.org)

[![Twitter Follow](https://img.shields.io/twitter/follow/holochain.svg?style=social&label=Follow)](https://twitter.com/holochain)
License: [![License: CAL 1.0](https://img.shields.io/badge/License-CAL%201.0-blue.svg)](https://github.com/holochain/cryptographic-autonomy-license)

Current version: 0.0.1

holo_hash::HoloHash is a hashing framework for Holochain.

Note that not all HoloHashes are simple hashes of the full content as you
might expect in a "content-addressable" application.

The main exception is AgentPubKey, which is simply the key itself to
enable self-proving signatures. As an exception it is also named exceptionally, i.e.
it doesn't end in "Hash". Another exception is DhtOps which sometimes hash either entry
content or header content to produce their hashes, depending on which type
of operation it is.

HoloHash implements `Display` providing a `to_string()` function accessing
the hash as a user friendly string. It also provides TryFrom for string
types allowing you to parse this string representation.

HoloHash includes a 4 byte (or u32) dht "location" that serves dual purposes.
 - It is used as a checksum when parsing string representations.
 - It is used as a u32 in our dht sharding algorithm.

HoloHash implements SerializedBytes to make it easy to cross ffi barriers
such as WASM and the UI websocket.

## Example

```rust
use holo_hash::*;
use std::convert::TryInto;
use holochain_serialized_bytes::SerializedBytes;

let entry: HoloHash =
    "uhCEkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm"
    .try_into()
    .unwrap();

assert_eq!(3860645936, entry.get_loc());

let bytes: SerializedBytes = entry.try_into().unwrap();

assert_eq!(
    "{\"type\":\"EntryContentHash\",\"hash\":[88,43,0,130,130,164,145,252,50,36,8,37,143,125,49,95,241,139,45,95,183,5,123,133,203,141,250,107,100,170,165,193,48,200,28,230]}",
    &format!("{:?}", bytes),
);
```

## Advanced

Calculating hashes takes time - In a futures context we don't want to block.
HoloHash provides sync (blocking) and async (non-blocking) apis for hashing.

```rust
use holo_hash::*;

let entry_content = b"test entry content";

let content_hash: HoloHash = EntryContentHash::with_data(entry_content.to_vec()).await.into();

assert_eq!(
    "EntryContentHash(uhCEkhPbA5vaw3Fk-ZvPSKuyyjg8eoX98fve75qiUEFgAE3BO7D4d)",
    &format!("{:?}", content_hash),
);
```

### Sometimes your data doesn't want to be re-hashed:

```rust
use holo_hash::*;

// pretend our pub key is all 0xdb bytes
let agent_pub_key = vec![0xdb; 32];

let agent_id: HoloHash = AgentPubKey::with_pre_hashed(agent_pub_key).into();

assert_eq!(
    "AgentPubKey(uhCAk29vb29vb29vb29vb29vb29vb29vb29vb29vb29vb29uTp5Iv)",
    &format!("{:?}", agent_id),
);
```

## Contribute
Holochain is an open source project.  We welcome all sorts of participation and are actively working on increasing surface area to accept it.  Please see our [contributing guidelines](/CONTRIBUTING.md) for our general practices and protocols on participating in the community, as well as specific expectations around things like code formatting, testing practices, continuous integration, etc.

* Connect with us on our [forum](https://forum.holochain.org)

## License
 [![License: CAL 1.0](https://img.shields.io/badge/License-CAL-1.0-blue.svg)](https://github.com/holochain/cryptographic-autonomy-license)

Copyright (C) 2019 - 2020, Holochain Foundation

This program is free software: you can redistribute it and/or modify it under the terms of the license
provided in the LICENSE file (CAL-1.0).  This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
PURPOSE.
