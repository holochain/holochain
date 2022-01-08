//! A model of the probability that some region will contain different data
//! for two different agents.
//!
//! The process of selecting a partition of our arc is mainly driven by this
//! model.
//!
//! The model is very simple right now, but as we refine the model to more
//! closely match reality, we can expect gossip to improve.

pub struct ProbabilityModel;

/// Model of the probability of a region being consistent between any two agents
impl ProbabilityModel {
    pub fn diff_probability(&self, coords: &RegionCoords) -> f64 {
        todo!()
    }
}
