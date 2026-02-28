use crate::influxive::{
    writer::{
        types::{Backend, BackendFactory},
        Metric,
    },
    *,
};

struct TestBackend {
    test_start: std::time::Instant,
    buffer_count: usize,
    write_count: Arc<std::sync::atomic::AtomicUsize>,
}

impl TestBackend {
    pub fn new(
        test_start: std::time::Instant,
        write_count: Arc<std::sync::atomic::AtomicUsize>,
    ) -> Self {
        Self {
            test_start,
            buffer_count: 0,
            write_count,
        }
    }
}

impl Backend for TestBackend {
    fn buffer_metric(&mut self, _metric: Metric) {
        self.buffer_count += 1;
        println!(
            "@@@ {:0.2} - buffer {}",
            self.test_start.elapsed().as_secs_f64(),
            self.buffer_count
        );
    }

    fn buffer_count(&self) -> usize {
        self.buffer_count
    }

    fn send(
        &mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + '_ + Send + Sync>> {
        Box::pin(async move {
            // simulate it taking a while to do things
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            self.write_count
                .fetch_add(self.buffer_count, std::sync::atomic::Ordering::SeqCst);
            self.buffer_count = 0;

            println!(
                "@@@ {:0.2} - write",
                self.test_start.elapsed().as_secs_f64()
            );
        })
    }
}

#[derive(Debug)]
struct TestFactory {
    test_start: std::time::Instant,
    write_count: Arc<std::sync::atomic::AtomicUsize>,
}

impl TestFactory {
    pub fn new(test_start: std::time::Instant) -> Arc<Self> {
        Arc::new(Self {
            test_start,
            write_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        })
    }

    pub fn get_write_count(&self) -> usize {
        self.write_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl BackendFactory for TestFactory {
    fn with_token_auth(
        &self,
        _host: String,
        _bucket: String,
        _token: String,
    ) -> Box<dyn Backend + 'static + Send + Sync> {
        let out: Box<dyn Backend + 'static + Send + Sync> =
            Box::new(TestBackend::new(self.test_start, self.write_count.clone()));
        out
    }
}

/// Setup InfluxiveWriter to use LineProtocolFileBackendFactory
fn create_file_writer(temp_dir: &tempfile::TempDir) -> (std::path::PathBuf, InfluxiveWriter) {
    std::fs::create_dir_all(temp_dir).unwrap();
    let test_path = temp_dir
        .path()
        .join(std::path::PathBuf::from("test_metrics.influx"));
    let mut config = InfluxiveWriterConfig::create_with_influx_file(test_path.clone());
    config.batch_duration = std::time::Duration::from_millis(30);
    let writer = InfluxiveWriter::with_token_auth(config, "", "", "");
    (test_path, writer)
}

#[tokio::test(flavor = "multi_thread")]
async fn writer_file_one() {
    use std::io::BufRead;

    let temp_dir = tempfile::TempDir::new().unwrap();
    let (test_path, writer) = create_file_writer(&temp_dir);
    // File should start empty
    let file = std::fs::File::open(&test_path).unwrap();
    let reader = std::io::BufReader::new(file);
    let res = reader.lines().next().transpose().unwrap();
    assert!(res.is_none());
    // Write one metric
    writer.write_metric(
        Metric::new(std::time::SystemTime::now(), "my.metric")
            .with_field("val", 3.77)
            .with_tag("tag", "test-tag"),
    );
    // File should still be empty since writer.send() not processed yet
    let file = std::fs::File::open(&test_path).unwrap();
    let reader = std::io::BufReader::new(file);
    let res = reader.lines().next().transpose().unwrap();
    assert!(res.is_none());
    // Wait for the batch process to trigger
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let file = std::fs::File::open(&test_path).unwrap();
    let reader = std::io::BufReader::new(file);
    let res = reader.lines().next().transpose().unwrap();
    assert!(res.is_some());
    let line = res.unwrap();
    let split = line.split(',').collect::<Vec<&str>>();
    assert_eq!(split[0], "my.metric");
    assert!(split[1].split(' ').collect::<Vec<&str>>()[1].contains("3.77"));
}

#[tokio::test(flavor = "multi_thread")]
async fn writer_file_many() {
    use std::io::BufRead;

    let temp_dir = tempfile::TempDir::new().unwrap();
    let (test_path, writer) = create_file_writer(&temp_dir);

    // File should start empty
    let file = std::fs::File::open(&test_path).unwrap();
    let reader = std::io::BufReader::new(file);
    let res = reader.lines().next().transpose().unwrap();
    assert!(res.is_none());

    // Write one metric
    writer.write_metric(
        Metric::new(std::time::SystemTime::now(), "my-metric")
            .with_field("f1", 1.77)
            .with_field("f2", 2.77)
            .with_field("f3", 3.77)
            .with_tag("tag", "test-tag"),
    );
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Write many metrics
    for n in 0..10 {
        writer.write_metric(
            Metric::new(std::time::SystemTime::now(), "my-second-metric")
                .with_field("val", n)
                .with_tag("tag1", "test-tag1")
                .with_tag("tag2", "test-tag2"),
        );
    }
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let file = std::fs::File::open(&test_path).unwrap();
    let reader = std::io::BufReader::new(file);
    let count = reader.lines().count();
    assert_eq!(count, 11);
}

#[tokio::test(flavor = "multi_thread")]
async fn writer_file_all_data_types() {
    use crate::influxive::types::{DataType, Metric};
    use std::io::BufRead;

    let temp_dir = tempfile::TempDir::new().unwrap();
    let (test_path, writer) = create_file_writer(&temp_dir);

    // Write metrics with different data types
    let test_cases = vec![
        ("bool_field", DataType::Bool(true)),
        ("float_field", DataType::F64(42.5)),
        ("int_field", DataType::I64(-42)),
        ("uint_field", DataType::U64(42)),
        (
            "string_field",
            DataType::String("test value".to_string()),
        ),
        (
            "quote_field",
            DataType::String("a \"test\" value".to_string()),
        ),
    ];

    for (field_name, value) in test_cases {
        writer.write_metric(
            Metric::new(std::time::SystemTime::UNIX_EPOCH, "test_metric")
                .with_field(field_name, value),
        );
    }

    // Wait for the batch to be written
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Read and verify the output
    let file = std::fs::File::open(&test_path).unwrap();
    let reader = std::io::BufReader::new(file);
    let lines: Vec<String> = reader.lines().collect::<Result<_, _>>().unwrap();

    assert_eq!(lines.len(), 6, "Expected 6 lines of metrics");

    // Verify each line contains the correct field value format
    assert!(
        lines.iter().any(|line| line.contains("bool_field=true")),
        "Boolean field not found"
    );
    assert!(
        lines.iter().any(|line| line.contains("float_field=42.5")),
        "Float field not found"
    );
    assert!(
        lines.iter().any(|line| line.contains("int_field=-42i")),
        "Integer field not found"
    );
    assert!(
        lines.iter().any(|line| line.contains("uint_field=42u")),
        "Unsigned integer field not found"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains(r#"string_field="test value""#)),
        "String field not found"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains(r#"quote_field="a \"test\" value"#)),
        "String field not found"
    );

    // Verify metric name and timestamp format for one line
    let first_line = &lines[0];
    assert!(
        first_line.starts_with("test_metric "),
        "Incorrect metric name format"
    );
    assert!(first_line.ends_with(" 0"), "Incorrect timestamp format"); // UNIX_EPOCH timestamp should be 0
}

#[tokio::test(flavor = "multi_thread")]
async fn writer_stress() {
    let test_start = std::time::Instant::now();

    let factory = TestFactory::new(test_start);

    let config = InfluxiveWriterConfig {
        batch_duration: std::time::Duration::from_millis(30),
        batch_buffer_size: 10,
        backend: factory.clone(),
    };

    let writer = InfluxiveWriter::with_token_auth(config, "", "", "");

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    println!(
        "@@@ {:0.2} - start easy",
        test_start.elapsed().as_secs_f64()
    );

    let mut cnt = 0;

    // this should be well within our cadence
    for _ in 0..5 {
        for _ in 0..5 {
            cnt += 1;
            println!(
                "@@@ {:0.2} - submit {}",
                test_start.elapsed().as_secs_f64(),
                cnt
            );
            writer.write_metric(
                Metric::new(std::time::SystemTime::now(), "my.metric")
                    .with_field("val", 3.77)
                    .with_tag("tag", "test-tag"),
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    assert_eq!(25, factory.get_write_count());

    println!(
        "@@@ {:0.2} - start stress",
        test_start.elapsed().as_secs_f64()
    );

    // this should be well outside our cadence
    for _ in 0..5 {
        for _ in 0..50 {
            cnt += 1;
            println!(
                "@@@ {:0.2} - submit {}",
                test_start.elapsed().as_secs_f64(),
                cnt
            );
            writer.write_metric(
                Metric::new(std::time::SystemTime::now(), "my.metric")
                    .with_field("val", 3.77)
                    .with_tag("tag", "test-tag"),
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    assert!(factory.get_write_count() < 250);
}
