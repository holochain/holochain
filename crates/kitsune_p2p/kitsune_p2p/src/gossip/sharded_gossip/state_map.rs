use super::*;

/// Map of gossip round state that checks for timed out rounds on gets.
#[derive(Default, Debug)]
pub(super) struct RoundStateMap {
    map: HashMap<StateKey, RoundState>,
    timed_out: Vec<(StateKey, RoundState)>,
}

impl RoundStateMap {
    /// Check if round has timed out and remove it if it has.
    pub(super) fn check_timeout(&mut self, key: &StateKey) -> bool {
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
    pub(super) fn get(&mut self, key: &StateKey) -> Option<&RoundState> {
        self.touch(key);
        self.map.get(key)
    }

    /// Get the mutable state if it hasn't timed out.
    pub(super) fn get_mut(&mut self, key: &StateKey) -> Option<&mut RoundState> {
        self.touch(key);
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
    pub(super) fn round_exists(&mut self, key: &StateKey) -> bool {
        self.check_timeout(key);
        self.map.contains_key(key)
    }

    /// Get all timed out rounds.
    pub(super) fn take_timed_out_rounds(&mut self) -> Vec<(StateKey, RoundState)> {
        std::mem::take(&mut self.timed_out)
    }

    /// Get the set of all locked regions for all rounds
    pub fn locked_regions(&self) -> HashSet<RegionCoords> {
        self.map
            .values()
            .flat_map(|r| r.locked_regions.clone())
            .collect()
    }

    /// Touch a round to reset its timeout.
    fn touch(&mut self, key: &StateKey) {
        if let Some(state) = self.map.get_mut(key) {
            state.last_touch = Instant::now();
        }
    }
}

impl From<HashMap<StateKey, RoundState>> for RoundStateMap {
    fn from(map: HashMap<StateKey, RoundState>) -> Self {
        Self {
            map,
            ..Default::default()
        }
    }
}
