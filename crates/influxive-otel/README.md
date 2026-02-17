[![Project](https://img.shields.io/badge/project-holochain-blue)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/chat-forum%2eholochain%2enet-blue)](https://forum.holochain.org)
[![Chat](https://img.shields.io/badge/chat-chat%2eholochain%2enet-blue)](https://chat.holochain.org)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue)](https://opensource.org/licenses/MIT)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue)](https://www.apache.org/licenses/LICENSE-2.0)

<!-- cargo-rdme start -->

Opentelemetry metrics bindings for influxive-child-svc.

## Example

```rust
use influxive_writer::*;
use std::sync::Arc;

// create an influxive writer
let writer = InfluxiveWriter::with_token_auth(
    InfluxiveWriterConfig::default(),
    "http://127.0.0.1:8086",
    "my.bucket",
    "my.token",
);

// register the meter provider
opentelemetry_api::global::set_meter_provider(
    influxive_otel::InfluxiveMeterProvider::new(
        Default::default(),
        Arc::new(writer),
    )
);

// create a metric
let m = opentelemetry_api::global::meter("my.meter")
    .f64_histogram("my.metric")
    .init();

// make a recording
m.record(3.14, &[]);
```

<!-- cargo-rdme end -->
