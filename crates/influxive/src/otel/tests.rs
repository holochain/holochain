use super::*;
use crate::child_svc::*;
use std::{
    sync::{
        atomic::{AtomicU16, Ordering},
        Arc,
    },
    time::Duration,
};
use utils::*;

#[tokio::test(flavor = "multi_thread")]
async fn observable_interval() {
    let tmp = tempfile::tempdir().unwrap();
    let (svc, meter_provider) = setup(tmp.path()).await;

    let name = "counting_u64";
    let metric = meter_provider.meter("influxive").u64_counter(name).build();

    metric.add(1, &[]);

    let result = poll_query(&svc, name, "", 300, |r| {
        r.tables.len() == 1 && !r.tables[0].rows.is_empty() && r.tables[0].rows.len() <= 2
    })
    .await;
    assert_eq!(result.tables[0].get::<u64>(0, "_value"), 1);

    svc.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn u64_counter() {
    let tmp = tempfile::tempdir().unwrap();
    let (svc, meter_provider) = setup(tmp.path()).await;

    let name = "u64_counter";
    let metric = meter_provider.meter("influxive").u64_counter(name).build();

    metric.add(1, &[]);

    let result = poll_query(&svc, name, "|> last()", 300, |r| {
        r.tables.len() == 1 && !r.tables[0].rows.is_empty() && r.tables[0].rows.len() <= 2
    })
    .await;
    assert_eq!(result.tables[0].get::<u64>(0, "_value"), 1);

    for _ in 0..5 {
        metric.add(1, &[]);
    }

    poll_query(&svc, name, "|> last()", 300, |r| {
        r.tables.len() == 1
            && r.tables[0].rows.len() == 1
            && r.tables[0].get::<u64>(0, "_value") == 6
    })
    .await;

    svc.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn f64_histogram() {
    let tmp = tempfile::tempdir().unwrap();
    let (svc, meter_provider) = setup(tmp.path()).await;

    let name = "f64_histogram";
    let metric = meter_provider
        .meter("influxive")
        .f64_histogram(name)
        .build();

    metric.record(1.0, &[]);

    // Influx writes u64 values into one table and f64 values into another table.
    // Hence 2 tables are expected to be present.
    let result = poll_query(&svc, name, "", 300, |r| {
        r.tables.len() == 2
            && !r.tables[0].rows.is_empty()
            && r.tables[0].rows.len() <= 2
            && r.tables[1].rows.len() >= 3
            && r.tables[1].rows.len() <= 4
    })
    .await;

    assert_eq!(result.tables[0].get::<String>(0, "_measurement"), name);
    assert_eq!(result.tables[0].get::<String>(0, "_field"), "count");
    assert_eq!(result.tables[0].get::<u64>(0, "_value"), 1);

    assert_eq!(result.tables[1].get::<String>(0, "_measurement"), name);
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

    // Expect two rows now per field value.
    let result = poll_query(&svc, name, "", 300, |r| {
        r.tables.len() == 2
            && r.tables[0].rows.len() >= 2
            && r.tables[0].rows.len() <= 3
            && r.tables[1].rows.len() >= 6
            && r.tables[1].rows.len() <= 9
    })
    .await;

    assert_eq!(result.tables[0].get::<String>(0, "_field"), "count");
    assert_eq!(result.tables[0].get::<u64>(0, "_value"), 1);
    assert_eq!(result.tables[0].get::<String>(1, "_field"), "count");
    assert_eq!(result.tables[0].get::<u64>(1, "_value"), 11);

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

    svc.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn f64_observable_gauge() {
    let tmp = tempfile::tempdir().unwrap();
    let (svc, meter_provider) = setup(tmp.path()).await;

    let name = "f64_observable_gauge";

    let observed_value = AtomicU16::new(0);
    // Create observable gauge metric that records an increasing value when observed.
    meter_provider
        .meter("influxive")
        .f64_observable_gauge(name)
        .with_callback(move |observer| {
            let value = observed_value.fetch_add(1, Ordering::SeqCst);
            observer.observe(value as f64, &[]);
        })
        .build();

    let result = poll_query(&svc, name, "", 300, |r| {
        r.tables.len() == 1 && !r.tables[0].rows.is_empty() && r.tables[0].rows.len() <= 2
    })
    .await;
    assert_eq!(result.tables[0].get::<String>(0, "_field"), "gauge");
    assert_eq!(result.tables[0].get::<String>(0, "_measurement"), name);
    assert_eq!(result.tables[0].get::<f64>(0, "_value"), 0.0);

    // Wait for more gauge values to have been recorded.
    let result = poll_query(&svc, name, "", 500, |r| {
        r.tables.len() == 1 && r.tables[0].rows.len() >= 5 && r.tables[0].rows.len() <= 6
    })
    .await;
    assert_eq!(result.tables[0].get::<f64>(0, "_value"), 0.0);
    assert_eq!(result.tables[0].get::<f64>(1, "_value"), 1.0);
    assert_eq!(result.tables[0].get::<f64>(2, "_value"), 2.0);
    assert_eq!(result.tables[0].get::<f64>(3, "_value"), 3.0);
    assert_eq!(result.tables[0].get::<f64>(4, "_value"), 4.0);

    svc.shutdown();
}

async fn setup(tmp: &std::path::Path) -> (Arc<InfluxiveChildSvc>, InfluxiveMeterProvider) {
    let influxive_svc = Arc::new(
        InfluxiveChildSvc::new(
            InfluxiveChildSvcConfig::default()
                .with_database_path(Some(tmp.into()))
                .with_metric_write(InfluxiveWriterConfig::default().with_batch_buffer_size(1)),
        )
        .await
        .unwrap(),
    );
    let meter_provider = InfluxiveMeterProvider::new(
        InfluxiveMeterProviderConfig::default()
            .with_report_interval(Some(Duration::from_millis(100))),
        influxive_svc.clone(),
    );
    (influxive_svc, meter_provider)
}

async fn poll_query(
    svc: &InfluxiveChildSvc,
    measurement: &str,
    query_suffix: &str,
    timeout_ms: u64,
    condition: impl Fn(&QueryResult) -> bool,
) -> QueryResult {
    tokio::time::timeout(Duration::from_millis(timeout_ms), async {
        loop {
            let result = QueryResult::parse(
                &svc.query(format!(
                    r#"
from(bucket: "influxive")
|> range(start: -15m, stop: now())
|> filter(fn: (r) => r._measurement == "{measurement}")
{query_suffix}"#
                ))
                .await
                .unwrap(),
            );
            if condition(&result) {
                println!("{result:#?}");
                return result;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap()
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
                .unwrap_or_else(|| {
                    panic!(
                        "didn't find provided column {column} in row {}",
                        self.rows[row]
                    )
                });
            self.rows[row]
                .split(',')
                .nth(col_idx)
                .unwrap_or_else(|| panic!("didn't find column {col_idx}"))
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
