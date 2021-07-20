use super::*;

/// Map of gossip round state that checks for timed out rounds on gets.
#[derive(Default)]
pub(super) struct RoundStateMap {
    map: HashMap<StateKey, RoundState>,
}

impl RoundStateMap {
    /// Check if round has timed out and remove it if it has.
    pub(super) fn check_timeout(&mut self, key: &StateKey) -> bool {
        let mut timed_out = false;
        if let Some(state) = self.map.get(key) {
            if state.created_at.elapsed().as_millis() as u32 > state.round_timeout {
                self.map.remove(key);
                timed_out = true;
            }
        }
        timed_out
    }

    /// Get the state if it hasn't timed out.
    pub(super) fn get(&mut self, key: &StateKey) -> Option<&RoundState> {
        self.check_timeout(key);
        self.map.get(key)
    }

    /// Get the mutable state if it hasn't timed out.
    pub(super) fn get_mut(&mut self, key: &StateKey) -> Option<&mut RoundState> {
        self.check_timeout(key);
        self.map.get_mut(key)
    }

    /// Remove the state.
    pub(super) fn remove(&mut self, key: &StateKey) -> Option<RoundState> {
        self.map.remove(key)
    }

    /// Insert new state and return the old state if there was any.
    pub(super) fn insert(&mut self, key: StateKey, round_state: RoundState) -> Option<RoundState> {
        self.map.insert(key, round_state)
    }

    /// Get the set of current rounds and remove any expired rounds.
    pub(super) fn current_rounds(&mut self) -> HashSet<Tx2Cert> {
        self.map
            .retain(|_, v| (v.created_at.elapsed().as_millis() as u32) < v.round_timeout);
        self.map.keys().cloned().collect::<HashSet<_>>()
    }

    /// Check if a non-expired round exists.
    pub(super) fn round_exists(&mut self, key: &StateKey) -> bool {
        self.check_timeout(key);
        self.map.contains_key(key)
    }
}
