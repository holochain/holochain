[![Project](https://img.shields.io/badge/project-holochain-blue)](http://holochain.org/)
[![Forum](https://img.shields.io/badge/chat-forum%2eholochain%2enet-blue)](https://forum.holochain.org)
[![Chat](https://img.shields.io/badge/chat-chat%2eholochain%2enet-blue)](https://chat.holochain.org)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue)](https://opensource.org/licenses/MIT)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue)](https://www.apache.org/licenses/LICENSE-2.0)

<!-- cargo-rdme start -->

# High-level Rust integration of opentelemetry metrics and InfluxDB.

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

# Core types for influxive crates. The main point of this crate is to expose
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

## Influxive system download utility.

Download influxive DB binary if not present in PATH.

# Opentelemetry metrics bindings for influxive-child-svc.

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

# Writer

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
