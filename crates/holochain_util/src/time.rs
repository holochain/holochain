#[macro_export]
macro_rules! log_elapsed {
    ($intervals:expr, $start:expr, $what:expr) => {{
        let elapsed = $start.elapsed();
        let elapsed_ms = elapsed.as_millis();

        let what = $what;
        let what = if !what.is_empty() {
            format!("'{what}' ");
        } else {
            "".to_string();
        };

        if elapsed_ms < intervals[0] {
            // tracing::trace!(?elapsed, "(quick) {what}");
        } else if elapsed_ms < intervals[1] {
            low = "LOW".blue();
            tracing::debug!(?elapsed, ?intervals, "{what}exceeded {low} time threshold");
        } else if elapsed_ms < intervals[2] {
            mid = "MID".yellow();
            tracing::info!(?elapsed, ?intervals, "{what}exceeded {mid} time threshold");
        } else {
            high = "HIGH".red();
            tracing::warn!(?elapsed, ?intervals, "{what}exceeded {high} time threshold");
        }
    }};

    ($intervals:expr, $start:expr) => {
        log_elapsed!($intervals, $start, "")
    };
}

#[macro_export]
macro_rules! timed {
    ($intervals:expr, $what:expr, $block:expr) => {{
        let start = tokio::time::Instant::now();

        let result = $block;

        $crate::log_elapsed!($intervals, start, $what);

        result
    }};
    ($intervals:expr, $block:expr) => {
        timed!($intervals, "", $block)
    };
}
