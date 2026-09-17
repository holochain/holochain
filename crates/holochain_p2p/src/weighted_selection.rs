use crate::peer_latency_store::LatencyData;
use kitsune2_api::Url;
use rand::prelude::IndexedRandom;

/// Selects `count` URLs using latency-aware weighted random selection without replacement.
///
/// URLs flagged by the latency store as having failed pings are filtered out
/// before selection so they cannot receive real traffic while they fail
/// health pings. Returns an empty `Vec` if every input URL has failed pings.
///
/// Lower-latency URLs receive higher weights (`1 / latency_ms`). URLs
/// without a latency estimate receive the median weight of known URLs.
/// Falls back to uniform random if no latency data exists for any URL.
pub(crate) fn select_weighted_urls(store: &LatencyData, urls: &[Url], count: usize) -> Vec<Url> {
    if urls.is_empty() || count == 0 {
        return Vec::new();
    }

    let selectable: Vec<Url> = urls
        .iter()
        .filter(|url| !store.has_failed_pings(url))
        .cloned()
        .collect();

    if selectable.is_empty() {
        return Vec::new();
    }

    let count = count.min(selectable.len());

    let mut known_weights: Vec<f64> = selectable
        .iter()
        .filter_map(|url| store.get_weight(url))
        .collect();

    if known_weights.is_empty() {
        return selectable
            .choose_multiple(&mut rand::rng(), count)
            .cloned()
            .collect();
    }

    known_weights.sort_by(f64::total_cmp);
    let median_weight = if known_weights.len().is_multiple_of(2) {
        let upper = known_weights.len() / 2;
        (known_weights[upper - 1] + known_weights[upper]) / 2.0
    } else {
        known_weights[known_weights.len() / 2]
    };

    selectable
        .choose_multiple_weighted(&mut rand::rng(), count, |url| {
            store.get_weight(url).unwrap_or(median_weight)
        })
        .map(|selection| selection.cloned().collect())
        .unwrap_or_else(|_| {
            selectable
                .choose_multiple(&mut rand::rng(), count)
                .cloned()
                .collect()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::time::Duration;

    fn test_url(name: &str) -> Url {
        Url::from_str(format!("ws://test-{name}:1234")).unwrap()
    }

    /// Drives `record_failure` until the URL crosses the ping-failure
    /// threshold without depending on the private constant.
    fn mark_ping_failed(store: &mut LatencyData, url: &Url) {
        for _ in 0..100 {
            if store.has_failed_pings(url) {
                return;
            }
            store.record_failure(url);
        }
        panic!("failed to mark url as ping-failed within 100 failures");
    }

    #[test]
    fn all_unknown_urls_uses_uniform_selection() {
        let store = LatencyData::default();
        let urls = vec![test_url("1"), test_url("2"), test_url("3")];

        let selected = select_weighted_urls(&store, &urls, 2);
        assert_eq!(selected.len(), 2);
    }

    #[test]
    fn low_latency_url_selected_more_often() {
        let mut store = LatencyData::default();
        let fast_url = test_url("fast");
        let slow_url = test_url("slow");

        store.record_sample(fast_url.clone(), Duration::from_millis(10));
        store.record_sample(slow_url.clone(), Duration::from_millis(1000));

        let urls = vec![fast_url.clone(), slow_url];

        let mut fast_count = 0;
        for _ in 0..1000 {
            let selected = select_weighted_urls(&store, &urls, 1);
            if selected[0] == fast_url {
                fast_count += 1;
            }
        }

        assert!(
            fast_count > 900,
            "fast url selected {fast_count}/1000 times"
        );
    }

    #[test]
    fn unknown_urls_get_median_weight() {
        let mut store = LatencyData::default();
        let known_url = test_url("known");
        let unknown_url = test_url("unknown");
        store.record_sample(known_url.clone(), Duration::from_millis(100));

        let urls = vec![known_url, unknown_url.clone()];

        let mut unknown_count = 0;
        for _ in 0..1000 {
            let selected = select_weighted_urls(&store, &urls, 1);
            if selected[0] == unknown_url {
                unknown_count += 1;
            }
        }

        assert!(
            (300..700).contains(&unknown_count),
            "unknown url selected {unknown_count}/1000 times"
        );
    }

    #[test]
    fn zero_latency_clamped_to_one_ms() {
        let mut store = LatencyData::default();
        let url = test_url("zero");
        store.record_sample(url.clone(), Duration::ZERO);

        let selected = select_weighted_urls(&store, &[url], 1);
        assert_eq!(selected.len(), 1);
    }

    #[test]
    fn single_url_always_selected() {
        let store = LatencyData::default();
        let selected = select_weighted_urls(&store, &[test_url("1")], 1);

        assert_eq!(selected.len(), 1);
    }

    #[test]
    fn request_more_than_available_returns_all() {
        let store = LatencyData::default();
        let urls = vec![test_url("1"), test_url("2")];

        let selected = select_weighted_urls(&store, &urls, 5);
        assert_eq!(selected.len(), 2);
    }

    #[test]
    fn duplicate_urls_are_preserved() {
        let mut store = LatencyData::default();
        let url1 = test_url("1");
        let url2 = test_url("2");

        store.record_sample(url1.clone(), Duration::from_millis(10));
        store.record_sample(url2.clone(), Duration::from_millis(10));

        // Caller is responsible for deduplication; this function
        // just selects from the given slice.
        let urls = vec![url1, url2];
        let selected = select_weighted_urls(&store, &urls, 2);
        let unique: HashSet<_> = selected.into_iter().collect();
        assert_eq!(unique.len(), 2);
    }

    #[test]
    fn ping_failed_urls_are_excluded() {
        let mut store = LatencyData::default();
        let healthy = test_url("healthy");
        let dead = test_url("dead");

        store.record_sample(healthy.clone(), Duration::from_millis(10));
        mark_ping_failed(&mut store, &dead);

        let urls = vec![healthy.clone(), dead.clone()];
        for _ in 0..50 {
            let selected = select_weighted_urls(&store, &urls, 1);
            assert_eq!(selected, vec![healthy.clone()]);
        }
    }

    #[test]
    fn all_ping_failed_returns_empty() {
        let mut store = LatencyData::default();
        let a = test_url("a");
        let b = test_url("b");
        mark_ping_failed(&mut store, &a);
        mark_ping_failed(&mut store, &b);

        let selected = select_weighted_urls(&store, &[a, b], 2);
        assert!(selected.is_empty());
    }

    #[test]
    fn ping_failed_count_does_not_consume_selection_slot() {
        let mut store = LatencyData::default();
        let healthy1 = test_url("h1");
        let healthy2 = test_url("h2");
        let dead = test_url("dead");

        store.record_sample(healthy1.clone(), Duration::from_millis(10));
        store.record_sample(healthy2.clone(), Duration::from_millis(10));
        mark_ping_failed(&mut store, &dead);

        let urls = vec![healthy1, healthy2, dead];
        let selected = select_weighted_urls(&store, &urls, 3);
        // Only 2 selectable URLs remain after filtering the dead one.
        assert_eq!(selected.len(), 2);
    }
}
