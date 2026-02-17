[![Project](https://img.shields.io/badge/project-holochain-blue)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/chat-forum%2eholochain%2enet-blue)](https://forum.holochain.org)
[![Chat](https://img.shields.io/badge/chat-chat%2eholochain%2enet-blue)](https://chat.holochain.org)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue)](https://opensource.org/licenses/MIT)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue)](https://www.apache.org/licenses/LICENSE-2.0)

<!-- cargo-rdme start -->

Rust utility for efficiently writing metrics to InfluxDB.
Metrics can be written directly to a running InfluxDB instance or
written to a Line Protocol file on disk that can be pushed to InfluxDB using Telegraf.

## Example

### Writing to a running InfluxDB instance

```rust
use influxive_core::Metric;
use influxive_writer::*;

let writer = InfluxiveWriter::with_token_auth(
    InfluxiveWriterConfig::default(),
    "http://127.0.0.1:8086",
    "my.bucket",
    "my.token",
);

writer.write_metric(
    Metric::new(
        std::time::SystemTime::now(),
        "my.metric",
    )
    .with_field("value", 3.14)
    .with_tag("tag", "test-tag")
);
```

### Writing to a file on disk

```rust
use influxive_core::Metric;
use influxive_writer::*;

let path = std::path::PathBuf::from("my-metrics.influx");
let config = InfluxiveWriterConfig::create_with_influx_file(path.clone());
// The file backend ignores host/bucket/token
let writer = InfluxiveWriter::with_token_auth(config, "", "", "");

writer.write_metric(
    Metric::new(
        std::time::SystemTime::now(),
        "my.metric",
    )
    .with_field("value", 3.14)
    .with_tag("tag", "test-tag")
);

// Now you can read and use the metrics file `my-metrics.influx`

```

<!-- cargo-rdme end -->
