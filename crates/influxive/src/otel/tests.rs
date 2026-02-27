use super::*;
use crate::child_svc::*;
use opentelemetry::KeyValue;
use std::{str::FromStr, sync::Arc};
use utils::*;

// to be implemented:
// f64_observable_gauge

#[tokio::test(flavor = "multi_thread")]
async fn observable_interval() {
    let tmp = tempfile::tempdir().unwrap();

    let influxive_svc = Arc::new(
        InfluxiveChildSvc::new(
            InfluxiveChildSvcConfig::default()
                .with_database_path(Some(tmp.path().into()))
                .with_metric_write(
                    // pass every metric directly to the writer
                    InfluxiveWriterConfig::default().with_batch_buffer_size(1),
                ),
        )
        .await
        .unwrap(),
    );

    let meter_provider = InfluxiveMeterProvider::new(
        InfluxiveMeterProviderConfig::default()
            .with_observable_report_interval(Some(Duration::from_millis(100))),
        influxive_svc.clone(),
    );

    let counter = "counting_u64";

    let metric = meter_provider
        .meter("influxive")
        .u64_counter(counter)
        .build();

    metric.add(1, &[]);

    // Wait for metrics to be written to Influx.
    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = influxive_svc
        .query(format!(
            r#"
from(bucket: "influxive")
|> range(start: -15m, stop: now())
|> filter(fn: (r) => r._measurement == "{counter}")
"#
        ))
        .await
        .unwrap();
    let query_result = QueryResult::parse(&result);

    assert_eq!(query_result.tables.len(), 1);
    assert_eq!(query_result.tables[0].rows.len(), 1);
    assert_eq!(query_result.tables[0].get::<u64>(0, "_value"), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn u64_counter() {
    let tmp = tempfile::tempdir().unwrap();

    let influxive_svc = Arc::new(
        InfluxiveChildSvc::new(
            InfluxiveChildSvcConfig::default()
                .with_database_path(Some(tmp.path().into()))
                .with_metric_write(InfluxiveWriterConfig::default().with_batch_buffer_size(1)),
        )
        .await
        .unwrap(),
    );

    let meter_provider = InfluxiveMeterProvider::new(
        InfluxiveMeterProviderConfig::default()
            .with_observable_report_interval(Some(std::time::Duration::from_millis(100))),
        influxive_svc.clone(),
    );

    let name = "u64_counter";

    let metric = meter_provider.meter("influxive").u64_counter(name).build();

    metric.add(1, &[]);

    // Wait for report interval to elapse and data to be written.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let result = QueryResult::parse(
        &influxive_svc
            .query(format!(
                r#"
from(bucket: "influxive")
|> range(start: -15m, stop: now())
|> filter(fn: (r) => r._measurement == "{name}")
|> last()
"#
            ))
            .await
            .unwrap(),
    );

    assert_eq!(result.tables.len(), 1);
    assert_eq!(result.tables[0].rows.len(), 1);
    assert_eq!(result.tables[0].get::<u64>(0, "_value"), 1, "{result:#?}");

    for _ in 0..5 {
        metric.add(1, &[]);
    }

    // Wait for report interval to elapse and data to be written.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let result = QueryResult::parse(
        &influxive_svc
            .query(format!(
                r#"
from(bucket: "influxive")
|> range(start: -15m, stop: now())
|> filter(fn: (r) => r._measurement == "{name}")
|> last()
"#
            ))
            .await
            .unwrap(),
    );

    assert_eq!(result.tables.len(), 1);
    assert_eq!(result.tables[0].rows.len(), 1);
    assert_eq!(
        result.tables[0].get::<u64>(0, "_value"),
        6,
        "result {result:#?}"
    );

    influxive_svc.shutdown();
    drop(influxive_svc);
}

#[tokio::test(flavor = "multi_thread")]
async fn f64_histogram() {
    let tmp = tempfile::tempdir().unwrap();

    let influxive_svc = Arc::new(
        InfluxiveChildSvc::new(
            InfluxiveChildSvcConfig::default()
                .with_database_path(Some(tmp.path().into()))
                .with_metric_write(InfluxiveWriterConfig::default().with_batch_buffer_size(1)),
        )
        .await
        .unwrap(),
    );

    let meter_provider = InfluxiveMeterProvider::new(
        InfluxiveMeterProviderConfig::default()
            .with_observable_report_interval(Some(std::time::Duration::from_millis(100))),
        influxive_svc.clone(),
    );

    let name = "f64_histogram";

    let metric = meter_provider
        .meter("influxive")
        .f64_histogram(name)
        .build();

    metric.record(1.0, &[]);

    // Wait for report interval to elapse and data to be written.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let result = QueryResult::parse(
        &influxive_svc
            .query(format!(
                r#"
from(bucket: "influxive")
|> range(start: -15m, stop: now())
|> filter(fn: (r) => r._measurement == "{name}")
"#
            ))
            .await
            .unwrap(),
    );

    // Influx writes u64 values into one table and f64 values into another table.
    // Hence 2 tables are expected to be present.
    assert_eq!(result.tables.len(), 2);
    assert_eq!(result.tables[0].rows.len(), 1);
    assert_eq!(result.tables[0].get::<String>(0, "_measurement"), name);

    assert_eq!(result.tables[0].get::<String>(0, "_field"), "count");
    assert_eq!(result.tables[0].get::<u64>(0, "_value"), 1);

    assert_eq!(result.tables[1].get::<String>(0, "_measurement"), name);

    assert_eq!(result.tables[1].rows.len(), 3);
    assert_eq!(result.tables[1].get::<String>(0, "_field"), "max");
    assert_eq!(result.tables[1].get::<f64>(0, "_value"), 1.0);
    assert_eq!(result.tables[1].get::<String>(1, "_field"), "min");
    assert_eq!(result.tables[1].get::<f64>(1, "_value"), 1.0);
    assert_eq!(result.tables[1].get::<String>(2, "_field"), "sum");
    assert_eq!(result.tables[1].get::<f64>(2, "_value"), 1.0);

    // Record many metrics at once and check that only one export to Influx happens.
    for i in 0..10 {
        metric.record(f64::from(i), &[]);
    }

    // Wait for report interval to elapse and data to be written.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Return all rows, not just the last one, to check that the writer
    // only writer per specified interval and not for every recorded metric.
    let result = QueryResult::parse(
        &influxive_svc
            .query(format!(
                r#"
from(bucket: "influxive")
|> range(start: -15m, stop: now())
|> filter(fn: (r) => r._measurement == "{name}")
"#
            ))
            .await
            .unwrap(),
    );

    assert_eq!(result.tables.len(), 2);
    // Expect two rows now per table per value.
    assert_eq!(result.tables[0].rows.len(), 2);
    assert_eq!(result.tables[0].get::<String>(0, "_field"), "count");
    assert_eq!(result.tables[0].get::<u64>(0, "_value"), 1);
    assert_eq!(result.tables[0].get::<String>(1, "_field"), "count");
    assert_eq!(result.tables[0].get::<u64>(1, "_value"), 11);

    assert_eq!(result.tables[1].rows.len(), 6);
    assert_eq!(result.tables[1].get::<String>(0, "_field"), "max");
    assert_eq!(result.tables[1].get::<f64>(0, "_value"), 1.0);
    assert_eq!(result.tables[1].get::<String>(1, "_field"), "max");
    assert_eq!(result.tables[1].get::<f64>(1, "_value"), 9.0);
    assert_eq!(result.tables[1].get::<String>(2, "_field"), "min");
    assert_eq!(result.tables[1].get::<f64>(2, "_value"), 1.0);
    assert_eq!(result.tables[1].get::<String>(3, "_field"), "min");
    assert_eq!(result.tables[1].get::<f64>(3, "_value"), 0.0);
    assert_eq!(result.tables[1].get::<String>(4, "_field"), "sum");
    assert_eq!(result.tables[1].get::<f64>(4, "_value"), 1.0);
    assert_eq!(result.tables[1].get::<String>(5, "_field"), "sum");
    assert_eq!(result.tables[1].get::<f64>(5, "_value"), 46.0);

    influxive_svc.shutdown();
    drop(influxive_svc);
}

mod utils {
    use std::str::FromStr;

    const EXAMPLE_QUERY_RESULT: &str = r#"
#group,false,false,true,true,false,false,true,true,true
#datatype,string,long,dateTime:RFC3339,dateTime:RFC3339,dateTime:RFC3339,long,string,string,string
#default,_result,,,,,,,,
,result,table,_start,_stop,_time,_value,_field,_measurement,key1
,,0,2026-02-26T21:45:57.167828262Z,2026-02-26T22:00:57.167828262Z,2026-02-26T22:00:57.164861261Z,1,sum_value,u64_counter,value1

#group,false,false,true,true,false,false,true,true,true
#datatype,string,long,dateTime:RFC3339,dateTime:RFC3339,dateTime:RFC3339,long,string,string,string
#default,_result,,,,,,,,
,result,table,_start,_stop,_time,_value,_field,_measurement,key2
,,0,2026-02-26T21:45:57.167828262Z,2026-02-26T22:00:57.167828262Z,2026-02-26T22:00:57.164861261Z,1,sum_value,u64_counter,value2
,,0,2026-02-26T21:45:57.167828262Z,2026-02-26T22:00:57.167828262Z,2026-02-26T22:00:57.164861261Z,1,sum_value,u64_counter,value2
"#;

    #[derive(Debug)]
    pub(super) struct QueryResult {
        pub(super) tables: Vec<Table>,
    }

    #[derive(Debug)]
    pub(super) struct Table {
        header: String,
        pub(super) rows: Vec<String>,
    }

    impl Table {
        pub(super) fn get<T>(&self, row: usize, column: &str) -> T
        where
            T: FromStr + std::fmt::Debug,
            T::Err: std::fmt::Debug,
        {
            let col_idx = self
                .header
                .split(',')
                .position(|h| h == column)
                .expect(&format!(
                    "didn't find provided column {column} in row {}",
                    self.rows[row]
                ));
            self.rows[row]
                .split(',')
                .nth(col_idx)
                .expect(&format!("didn't find column {col_idx}"))
                .parse()
                .unwrap()
        }
    }

    impl QueryResult {
        pub(super) fn parse(result: &str) -> Self {
            let mut groups = Vec::new();
            let mut current_header: Option<String> = None;
            let mut current_rows = Vec::new();

            for line in result.lines() {
                if line.is_empty() {
                    continue;
                }

                // #group marks start of a new table
                if line.starts_with("#group") {
                    if let Some(header) = current_header.take() {
                        groups.push(Table {
                            header,
                            rows: current_rows,
                        });
                        current_rows = Vec::new();
                    }
                } else if line.starts_with('#') {
                    // Skip other annotation lines (#datatype, #default)
                    continue;
                } else if line.starts_with(",result,table") {
                    // Header line
                    current_header = Some(line.to_string());
                } else if current_header.is_some() {
                    // Data row
                    current_rows.push(line.to_string());
                }
            }

            if let Some(header) = current_header {
                groups.push(Table {
                    header,
                    rows: current_rows,
                });
            }

            QueryResult { tables: groups }
        }
    }

    #[test]
    fn query_result_sanity() {
        let result = QueryResult::parse(EXAMPLE_QUERY_RESULT);
        assert_eq!(result.tables.len(), 2);
        assert_eq!(result.tables[0].rows.len(), 1);
        assert_eq!(result.tables[1].rows.len(), 2);
        assert_eq!(result.tables[0].get::<String>(0, "key1"), "value1");
    }
}
