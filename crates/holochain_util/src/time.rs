#[macro_export]
macro_rules! log_elapsed {
    ($intervals:expr, $start:expr) => {{
        log_elapsed!($intervals, $start, "")
    }};

    ($intervals:expr, $start:expr, $what:expr) => {{
        // use $crate::colored::Colorize;

        let intervals = $intervals;
        let elapsed = $start.elapsed();
        let elapsed_ms = elapsed.as_millis();

        let what: &str = $what;
        let what: String = if !what.is_empty() {
            format!("'{what}' ")
        } else {
            "".to_string()
        };

        if elapsed_ms < intervals[0] {
            // tracing::trace!(?elapsed, "(quick) {what}");
        } else if elapsed_ms < intervals[1] {
            tracing::info!(?elapsed, ?intervals, "{what}exceeded LOW time threshold",);
        } else if elapsed_ms < intervals[2] {
            tracing::warn!(?elapsed, ?intervals, "{what}exceeded MID time threshold",);
        } else {
            tracing::error!(?elapsed, ?intervals, "{what}exceeded HIGH time threshold",);
        }
    }};
}

#[macro_export]
macro_rules! timed {
    ($intervals:expr, $block:expr) => {{
        timed!($intervals, stringify!($block), $block)
    }};

    ($intervals:expr, $what:expr, $block:expr) => {{
        #[cfg(feature = "tokio")]
        let start = tokio::time::Instant::now();
        #[cfg(not(feature = "tokio"))]
        let start = std::time::Instant::now();

        let result = $block;

        $crate::log_elapsed!($intervals, start, $what);

        result
    }};
}

#[tokio::test]
async fn test_timed() {
    use std::sync::{Arc, Mutex};

    #[derive(Default, Clone)]
    struct Logs(Arc<Mutex<String>>);

    impl std::io::Write for Logs {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let mut logs = self.0.lock().unwrap();
            logs.push_str(&String::from_utf8_lossy(buf));
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Logs {
        pub fn get(&self) -> String {
            self.0.lock().unwrap().clone()
        }
    }

    let logs = Logs::default();
    let logs2 = logs.clone();

    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || logs2.clone())
        .init();

    tokio::time::pause();

    timed!([10, 20, 30], "ctx1", {
        tokio::time::advance(tokio::time::Duration::from_millis(15)).await;
    });
    assert!(logs.get().contains("ctx1"));
    assert!(logs.get().contains("LOW"));

    timed!([1000, 2000, 3000], {
        tokio::time::advance(tokio::time::Duration::from_millis(2001)).await;
    });
    assert!(logs.get().contains("MID"));
    assert!(logs.get().contains("from_millis(2001)"));

    timed!([1, 2, 3], {
        tokio::time::advance(tokio::time::Duration::from_millis(3)).await;
    });
    assert!(logs.get().contains("HIGH"));
    assert!(logs.get().contains("from_millis(3)"));
}
