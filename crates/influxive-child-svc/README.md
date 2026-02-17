[![Project](https://img.shields.io/badge/project-holochain-blue)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/chat-forum%2eholochain%2enet-blue)](https://forum.holochain.org)
[![Chat](https://img.shields.io/badge/chat-chat%2eholochain%2enet-blue)](https://chat.holochain.org)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue)](https://opensource.org/licenses/MIT)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue)](https://www.apache.org/licenses/LICENSE-2.0)

<!-- cargo-rdme start -->

Run influxd as a child process.

## Example

```rust
use influxive_core::Metric;
use influxive_child_svc::*;

let tmp = tempfile::tempdir().unwrap();

let influxive = InfluxiveChildSvc::new(
    InfluxiveChildSvcConfig::default()
        .with_database_path(Some(tmp.path().to_owned())),
).await.unwrap();

influxive.write_metric(
    Metric::new(
        std::time::SystemTime::now(),
        "my.metric",
    )
    .with_field("value", 3.14)
    .with_tag("tag", "test-tag")
);
```

<!-- cargo-rdme end -->
