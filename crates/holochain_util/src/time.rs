pub fn log_elapsed(intervals: [u128; 3], start: tokio::time::Instant, what: &str) {
    let elapsed = start.elapsed();
    let elapsed_ms = elapsed.as_millis();
    if elapsed_ms < intervals[0] {
        // tracing::trace!(?elapsed, "(quick) {what}");
    } else if elapsed_ms < intervals[1] {
        tracing::debug!(
            ?elapsed,
            ?intervals,
            "{what} exceeded the LOW time threshold"
        );
    } else if elapsed_ms < intervals[2] {
        tracing::info!(
            ?elapsed,
            ?intervals,
            "{what} exceeded the MID time threshold"
        );
    } else {
        tracing::warn!(
            ?elapsed,
            ?intervals,
            "{what} exceeded the HIGH time threshold"
        );
    }
}
