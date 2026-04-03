use crate::peer_latency_store::PeerLatencyStore;
use holo_hash::AgentPubKey;
use kitsune2_api::Url;
use rand::prelude::IndexedRandom;
use std::collections::HashMap;
use std::time::Duration;

/// Converts a latency duration to a selection weight.
///
/// Weight is `1 / latency_ms`, so lower-latency peers get higher weights.
/// Latency is clamped to a minimum of 1ms to avoid division by zero.
fn latency_to_weight_ms(latency: Duration) -> f64 {
    1.0 / (latency.as_secs_f64() * 1000.0).max(1.0)
}

/// Selects `count` peers using latency-aware weighted random selection without replacement.
///
/// Peers are deduplicated by URL before weighting to prevent a conductor
/// advertising multiple agents on the same URL from being overweighted.
/// Lower-latency peers receive higher weights (`1 / latency_ms`). Peers
/// without a latency estimate receive the median weight of known peers.
/// Falls back to uniform random if no latency data exists for any peer.
pub(crate) fn select_weighted_peers(
    store: &PeerLatencyStore,
    peers: &[(AgentPubKey, Url)],
    count: usize,
) -> Vec<(AgentPubKey, Url)> {
    if peers.is_empty() || count == 0 {
        return Vec::new();
    }

    let deduped: Vec<(AgentPubKey, Url)> = peers
        .iter()
        .fold(HashMap::new(), |mut by_url, (agent, url)| {
            by_url.entry(url.clone()).or_insert_with(|| agent.clone());
            by_url
        })
        .into_iter()
        .map(|(url, agent)| (agent, url))
        .collect();

    let count = count.min(deduped.len());
    if count == 0 {
        return Vec::new();
    }

    let mut known_weights: Vec<f64> = deduped
        .iter()
        .filter_map(|(_, url)| {
            store
                .get_latency(url)
                .or_else(|| store.get_latency_including_stale(url))
        })
        .map(latency_to_weight_ms)
        .collect();

    if known_weights.is_empty() {
        return deduped
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

    deduped
        .choose_multiple_weighted(&mut rand::rng(), count, |(_, url)| {
            store
                .get_latency(url)
                .or_else(|| store.get_latency_including_stale(url))
                .map(latency_to_weight_ms)
                .unwrap_or(median_weight)
        })
        .map(|selection| selection.cloned().collect())
        .unwrap_or_else(|_| {
            deduped
                .choose_multiple(&mut rand::rng(), count)
                .cloned()
                .collect()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn test_url(name: &str) -> Url {
        Url::from_str(format!("ws://test-{name}:1234")).unwrap()
    }

    fn agent(byte: u8) -> AgentPubKey {
        AgentPubKey::from_raw_32(vec![byte; 32])
    }

    #[test]
    fn all_unknown_peers_uses_uniform_selection() {
        let store = PeerLatencyStore::new();
        let peers = vec![
            (agent(1), test_url("1")),
            (agent(2), test_url("2")),
            (agent(3), test_url("3")),
        ];

        let selected = select_weighted_peers(&store, &peers, 2);
        assert_eq!(selected.len(), 2);
    }

    #[test]
    fn low_latency_peer_selected_more_often() {
        let mut store = PeerLatencyStore::new();
        let fast_url = test_url("fast");
        let slow_url = test_url("slow");

        store.record_sample(fast_url.clone(), Duration::from_millis(10));
        store.record_sample(slow_url.clone(), Duration::from_millis(1000));

        let peers = vec![(agent(1), fast_url.clone()), (agent(2), slow_url)];

        let mut fast_count = 0;
        for _ in 0..1000 {
            let selected = select_weighted_peers(&store, &peers, 1);
            if selected[0].1 == fast_url {
                fast_count += 1;
            }
        }

        assert!(
            fast_count > 900,
            "fast peer selected {fast_count}/1000 times"
        );
    }

    #[test]
    fn url_deduplication_prevents_overweighting() {
        let mut store = PeerLatencyStore::new();
        let url1 = test_url("1");
        let url2 = test_url("2");

        store.record_sample(url1.clone(), Duration::from_millis(10));
        store.record_sample(url2.clone(), Duration::from_millis(10));

        let peers = vec![
            (agent(1), url1.clone()),
            (agent(2), url1.clone()),
            (agent(3), url2.clone()),
        ];

        let selected = select_weighted_peers(&store, &peers, 2);
        let urls: HashSet<_> = selected.iter().map(|(_, url)| url.clone()).collect();

        assert_eq!(selected.len(), 2);
        assert_eq!(urls.len(), 2);
    }

    #[test]
    fn unknown_peers_get_median_weight() {
        let mut store = PeerLatencyStore::new();
        let known_url = test_url("known");
        let unknown_url = test_url("unknown");
        store.record_sample(known_url.clone(), Duration::from_millis(100));

        let peers = vec![(agent(1), known_url), (agent(2), unknown_url.clone())];

        let mut unknown_count = 0;
        for _ in 0..1000 {
            let selected = select_weighted_peers(&store, &peers, 1);
            if selected[0].1 == unknown_url {
                unknown_count += 1;
            }
        }

        assert!(
            (300..700).contains(&unknown_count),
            "unknown peer selected {unknown_count}/1000 times"
        );
    }

    #[test]
    fn zero_latency_clamped_to_one_ms() {
        let mut store = PeerLatencyStore::new();
        let url = test_url("zero");
        store.record_sample(url.clone(), Duration::ZERO);

        let selected = select_weighted_peers(&store, &[(agent(1), url)], 1);
        assert_eq!(selected.len(), 1);
    }

    #[test]
    fn single_peer_always_selected() {
        let store = PeerLatencyStore::new();
        let selected = select_weighted_peers(&store, &[(agent(1), test_url("1"))], 1);

        assert_eq!(selected.len(), 1);
    }

    #[test]
    fn request_more_than_available_returns_all() {
        let store = PeerLatencyStore::new();
        let peers = vec![(agent(1), test_url("1")), (agent(2), test_url("2"))];

        let selected = select_weighted_peers(&store, &peers, 5);
        assert_eq!(selected.len(), 2);
    }
}
