[![Project](https://img.shields.io/badge/project-holochain-blue)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/chat-forum%2eholochain%2enet-blue)](https://forum.holochain.org)
[![Chat](https://img.shields.io/badge/chat-chat%2eholochain%2enet-blue)](https://chat.holochain.org)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue)](https://opensource.org/licenses/MIT)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue)](https://www.apache.org/licenses/LICENSE-2.0)

<!-- cargo-rdme start -->

Core types for influxive crates. The main point of this crate is to expose
the [MetricWriter] trait to be used by downstream influxive crates.

## Example [Metric] type creation:

```rust
let _metric = influxive_core::Metric::new(std::time::SystemTime::now(), "my.name")
    .with_field("field.bool", true)
    .with_field("field.float", 3.14)
    .with_field("field.signed", -42)
    .with_field("field.unsigned", 42)
    .with_field("field.string", "string.value")
    .with_tag("tag.bool", true)
    .with_tag("tag.float", 3.14)
    .with_tag("tag.signed", -42)
    .with_tag("tag.unsigned", 42)
    .with_tag("tag.string", "string.value");
```

<!-- cargo-rdme end -->
