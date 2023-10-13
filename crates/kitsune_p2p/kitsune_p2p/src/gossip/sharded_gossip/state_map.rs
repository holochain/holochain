use super::*;

/// Map of gossip round state that checks for timed out rounds on gets.
#[derive(Default, Debug)]
pub(super) struct RoundStateMap {
    map: HashMap<NodeCert, RoundState>,
    timed_out: Vec<(NodeCert, RoundState)>,
}

impl RoundStateMap {
    /// Check if round has timed out and remove it if it has.
    pub(super) fn check_timeout(&mut self, key: &NodeCert) -> bool {
        let mut timed_out = false;
        let mut finished = false;
        if let Some(state) = self.map.get(key) {
            if state.last_touch.elapsed() > state.round_timeout {
                if let Some(v) = self.map.remove(key) {
                    self.timed_out.push((key.clone(), v));
                }
                timed_out = true;
            } else if state.is_finished() {
                finished = true;
            }
        }
        // MD: I added this just to be safe. It made a difference.
        if finished {
            self.map.remove(key);
        }
        timed_out
    }

    /// Get the state if it hasn't timed out.
    pub(super) fn get(&mut self, key: &NodeCert) -> Option<&RoundState> {
        self.touch(key);
        self.check_timeout(key);
        self.map.get(key)
    }

    /// Get the mutable state if it hasn't timed out.
    pub(super) fn get_mut(&mut self, key: &NodeCert) -> Option<&mut RoundState> {
        self.touch(key);
        self.check_timeout(key);
        self.map.get_mut(key)
    }

    /// Remove the state.
    pub(super) fn remove(&mut self, key: &NodeCert) -> Option<RoundState> {
        self.map.remove(key)
    }

    /// Insert new state and return the old state if there was any.
    pub(super) fn insert(&mut self, key: NodeCert, round_state: RoundState) -> Option<RoundState> {
        self.map.insert(key, round_state)
    }

    /// Get the set of current rounds and remove any expired rounds.
    pub(super) fn current_rounds(&mut self) -> HashSet<NodeCert> {
        for (k, v) in std::mem::take(&mut self.map) {
            if v.last_touch.elapsed() < v.round_timeout {
                self.map.insert(k, v);
            } else {
                self.timed_out.push((k, v));
            }
        }
        self.map.keys().cloned().collect::<HashSet<_>>()
    }

    /// Check if a non-expired round exists.
    pub(super) fn round_exists(&mut self, key: &NodeCert) -> bool {
        self.check_timeout(key);
        self.map.contains_key(key)
    }

    /// Get all timed out rounds.
    pub(super) fn take_timed_out_rounds(&mut self) -> Vec<(NodeCert, RoundState)> {
        std::mem::take(&mut self.timed_out)
    }

    /// Touch a round to reset its timeout.
    fn touch(&mut self, key: &NodeCert) {
        if let Some(state) = self.map.get_mut(key) {
            state.last_touch = Instant::now();
        }
    }
}

impl From<HashMap<NodeCert, RoundState>> for RoundStateMap {
    fn from(map: HashMap<NodeCert, RoundState>) -> Self {
        Self {
            map,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::dht_arc::DhtArcSet;
    use crate::gossip::sharded_gossip::state_map::RoundStateMap;
    use crate::gossip::sharded_gossip::{NodeCert, RoundState};
    use crate::NOISE;
    use arbitrary::{Arbitrary, Unstructured};
    use kitsune_p2p_types::Tx2Cert;
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn hold_round_state() {
        let mut u = Unstructured::new(&NOISE);
        let (mut state_map, key) = test_round_state_map_with_single_key(&mut u);
        assert!(state_map.round_exists(&key));

        let state = state_map.get(&key);
        assert!(state.is_some());
        assert_eq!(Duration::from_millis(5), state.unwrap().round_timeout);
    }

    #[test]
    fn remove_round_state() {
        let mut u = Unstructured::new(&NOISE);
        let (mut state_map, key) = test_round_state_map_with_single_key(&mut u);
        assert!(state_map.round_exists(&key));

        let removed = state_map.remove(&key);
        assert!(removed.is_some());

        assert!(!state_map.round_exists(&key));
    }

    #[test]
    fn modify_round_state() {
        let mut u = Unstructured::new(&NOISE);
        let (mut state_map, key) = test_round_state_map_with_single_key(&mut u);

        {
            let state = state_map.get_mut(&key).unwrap();
            state.id = "test-change-state".to_string();
        }

        assert_eq!(
            "test-change-state".to_string(),
            state_map.get(&key).unwrap().id
        );
    }

    #[test]
    fn round_state_times_out_after_round_timeout() {
        let mut u = Unstructured::new(&NOISE);
        let (mut state_map, key) = test_round_state_map_with_single_key(&mut u);

        {
            // We must use a zero timeout here, unlike the other tests which just set the last_touch in the past,
            // because `get` also performs a touch, which would undo that change.
            let state = state_map.get_mut(&key).unwrap();
            state.round_timeout = Duration::ZERO;
            assert!(state.last_touch.elapsed() > state.round_timeout);
        }

        let state = state_map.get(&key);
        assert!(state.is_none());
    }

    #[test]
    fn round_state_mut_times_out_after_round_timeout() {
        let mut u = Unstructured::new(&NOISE);
        let (mut state_map, key) = test_round_state_map_with_single_key(&mut u);

        {
            // We must use a zero timeout here, unlike the other tests which just set the last_touch in the past,
            // because `get` also performs a touch, which would undo that change.
            let state = state_map.get_mut(&key).unwrap();
            state.round_timeout = Duration::ZERO;
            assert!(state.last_touch.elapsed() > state.round_timeout);
        }

        let state = state_map.get_mut(&key);
        assert!(state.is_none());
    }

    #[test]
    fn round_state_does_not_time_out_if_fetched() {
        let mut u = Unstructured::new(&NOISE);
        let (mut state_map, key) = test_round_state_map_with_single_key(&mut u);

        {
            let state = state_map.get_mut(&key).unwrap();
            state.last_touch = state.last_touch - Duration::from_secs(10);
            // Should be marked as timed out on next `round_exists`
            assert!(state.last_touch.elapsed() > state.round_timeout);
        }

        // Reset the last_touch
        state_map.get(&key);

        // Just reset by getting, so this will not remove
        let exists = state_map.round_exists(&key);
        assert!(exists);

        let state = state_map.get(&key);
        assert!(state.is_some())
    }

    #[test]
    fn round_state_mut_does_not_time_out_if_fetched() {
        let mut u = Unstructured::new(&NOISE);
        let (mut state_map, key) = test_round_state_map_with_single_key(&mut u);

        {
            let state = state_map.get_mut(&key).unwrap();
            state.last_touch = state.last_touch - Duration::from_secs(10);
            // Should be marked as timed out on next `round_exists`
            assert!(state.last_touch.elapsed() > state.round_timeout);
        }

        // Reset the last_touch
        state_map.get_mut(&key);

        // Just reset by getting, so this will not remove
        let exists = state_map.round_exists(&key);
        assert!(exists);

        let state = state_map.get_mut(&key);
        assert!(state.is_some())
    }

    #[test]
    fn get_current_rounds_from_round_state() {
        let mut u = Unstructured::new(&NOISE);
        let (mut state_map, key_1) = test_round_state_map_with_single_key(&mut u);

        let key_2 = insert_new_state(&mut state_map, &mut u);
        let key_3 = insert_new_state(&mut state_map, &mut u);

        assert_eq!(3, state_map.current_rounds().len());

        // Mark the state for key_2 as timed out
        {
            let state = state_map.get_mut(&key_2).unwrap();
            state.last_touch = state.last_touch - Duration::from_secs(10);
            // Should be marked as timed out on next `round_exists`
            assert!(state.last_touch.elapsed() > state.round_timeout);
        }

        let mut expected_current = HashSet::new();
        expected_current.insert(key_1);
        expected_current.insert(key_3);

        assert_eq!(expected_current, state_map.current_rounds());
        assert_eq!(1, state_map.take_timed_out_rounds().len());
    }

    #[test]
    fn expired_rounds_can_only_be_fetched_from_round_state_once() {
        let mut u = Unstructured::new(&NOISE);
        let (mut state_map, key) = test_round_state_map_with_single_key(&mut u);

        {
            let state = state_map.get_mut(&key).unwrap();
            state.last_touch = state.last_touch - Duration::from_secs(10);
            // Should be marked as timed out on next `round_exists`
            assert!(state.last_touch.elapsed() > state.round_timeout);
        }

        assert!(!state_map.round_exists(&key));
        assert_eq!(1, state_map.take_timed_out_rounds().len());
        assert_eq!(0, state_map.take_timed_out_rounds().len());
    }

    #[test]
    fn round_state_removed_if_finished_on_get_mut() {
        let mut u = Unstructured::new(&NOISE);
        let (mut state_map, key) = test_round_state_map_with_single_key(&mut u);

        {
            let state = state_map.get_mut(&key).unwrap();
            state.received_all_incoming_op_blooms = true;
            state.regions_are_queued = true;
            assert!(state.is_finished());
        }

        let state = state_map.get_mut(&key);
        assert!(state.is_none());
    }

    #[test]
    fn round_state_removed_if_finished_on_get() {
        let mut u = Unstructured::new(&NOISE);
        let (mut state_map, key) = test_round_state_map_with_single_key(&mut u);

        {
            let state = state_map.get_mut(&key).unwrap();
            state.received_all_incoming_op_blooms = true;
            state.regions_are_queued = true;
            assert!(state.is_finished());
        }

        let state = state_map.get(&key);
        assert!(state.is_none());
    }

    fn test_round_state_map_with_single_key(u: &mut Unstructured) -> (RoundStateMap, NodeCert) {
        let mut state_map = RoundStateMap::default();
        let key = insert_new_state(&mut state_map, u);

        (state_map, key)
    }

    fn test_round_state() -> RoundState {
        RoundState::new(
            vec![],
            Arc::new(DhtArcSet::new_empty()),
            None,
            Duration::from_millis(5),
        )
    }

    fn insert_new_state(state_map: &mut RoundStateMap, u: &mut Unstructured) -> NodeCert {
        let cert = Tx2Cert::arbitrary(u).unwrap();
        let key: NodeCert = cert.into();
        state_map.insert(key.clone(), test_round_state());

        key
    }
}
