[![Project](https://img.shields.io/badge/project-holochain-blue)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/chat-forum%2eholochain%2enet-blue)](https://forum.holochain.org)
[![Chat](https://img.shields.io/badge/chat-chat%2eholochain%2enet-blue)](https://chat.holochain.org)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue)](https://opensource.org/licenses/MIT)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue)](https://www.apache.org/licenses/LICENSE-2.0)

<!-- cargo-rdme start -->

High-level Rust integration of opentelemetry metrics and InfluxDB.

## Examples

### Easy, zero-configuration InfluxDB as a child process

```rust
let tmp = tempfile::tempdir().unwrap();

// create our meter provider
let (_influxive, meter_provider) = influxive::influxive_child_process_meter_provider(
    influxive::InfluxiveChildSvcConfig::default()
        .with_database_path(Some(tmp.path().to_owned())),
    influxive::InfluxiveMeterProviderConfig::default(),
).await.unwrap();

// register our meter provider
opentelemetry_api::global::set_meter_provider(meter_provider);

// create a metric
let m = opentelemetry_api::global::meter("my.meter")
    .f64_histogram("my.metric")
    .init();

// make a recording
m.record(3.14, &[]);
```

### Connecting to an already running InfluxDB system process

```rust
// create our meter provider
let meter_provider = influxive::influxive_external_meter_provider_token_auth(
    influxive::InfluxiveWriterConfig::default(),
    influxive::InfluxiveMeterProviderConfig::default(),
    "http://127.0.0.1:8086",
    "my.bucket",
    "my.token",
);

// register our meter provider
opentelemetry_api::global::set_meter_provider(meter_provider);

// create a metric
let m = opentelemetry_api::global::meter("my.meter")
    .f64_histogram("my.metric")
    .init();

// make a recording
m.record(3.14, &[]);
```

### Writing to an influx file

```rust
// create our meter provider
let meter_provider = influxive::influxive_file_meter_provider(
    influxive::InfluxiveWriterConfig::create_with_influx_file(std::path::PathBuf::from("my-metrics.influx")),
    influxive::InfluxiveMeterProviderConfig::default(),
);

// register our meter provider
opentelemetry_api::global::set_meter_provider(meter_provider);

// create a metric
let m = opentelemetry_api::global::meter("my.meter")
    .f64_histogram("my.metric")
    .init();

// make a recording
m.record(3.14, &[]);

// Read and use data in "my-metrics.influx"

```

<!-- cargo-rdme end -->
