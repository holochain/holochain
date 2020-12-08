use super::TestConductorHandle;
use std::collections::HashMap;

/// A collection of TestConductorHandles
pub struct TestConductorSet {
    _conductors: HashMap<String, TestConductorHandle>,
}

impl TestConductorSet {
    /// Constructor
    pub fn new<I: IntoIterator<Item = (String, TestConductorHandle)>>(i: I) -> Self {
        Self {
            _conductors: i.into_iter().collect(),
        }
    }
}
