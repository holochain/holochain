#[derive(Debug, Clone)]
pub struct ArqStrat {
    /// The minimum coverage the DHT seeks to maintain.
    ///
    /// This is the whole purpose for arc resizing.
    pub min_coverage: f64,

    /// A multiplicative factor of the min coverage which defines a max.
    /// coverage. We want coverage to be between the min and max coverage.
    /// This is expressed in terms of a value > 0 and < 1. For instance,
    /// a min coverage of 50 with a buffer of 0.2 implies a max coverage of 60.
    pub buffer: f64,

    /// There is a point below which we never want to lower the power.
    /// The algorithm will crash if this is 0, but in reality we want this to be
    /// substantially higher.
    ///
    /// Our "quantum chunk size" is 2^(min_power).
    pub min_power: u8,

    /// There is a point above which we never want to raise the power.
    /// This is because we want even a full arq to contain a certain number of
    /// chunks.
    pub max_power: u8,

    /// If the standard deviation of the powers of each arq in this view is
    /// greater than this threshold, then we might do something different when
    /// it comes to our decision to requantize. For now, just print a warning.
    ///
    /// TODO: this can probably be expressed in terms of `max_power_diff`.
    pub power_std_dev_threshold: f64,

    /// If the difference between the arq's power and the median power of all
    /// peer arqs (including this one) is greater than this diff,
    /// then don't requantize:
    /// just keep growing or shrinking past the min/max chunks value.
    ///
    /// This parameter determines how likely it is for there to be a difference in
    /// chunk sizes between two agents' arqs. It establishes the tradeoff between
    /// the size of payloads that must be sent and the extra depth of Fenwick
    /// tree data that must be stored (to accomodate agents whose power is
    /// lower than ours).
    ///
    /// This parameter is also what allows an arq to shrink to zero in a
    /// reasonable number of steps. Without this limit on power diff, we would
    /// keep requantizing until the power was 0 before shrinking to the empty arc.
    /// We may shrink to zero if our neighborhood is significantly over-covered,
    /// which can happen if a number of peers decide to keep their coverage
    /// higher than the ideal equilibrium value.
    ///
    /// Note that this parameter does not guarantee that any agent's arq
    /// will have a power +/- this diff from our power, but we may decide to
    /// choose not to gossip with agents whose power falls outside the range
    /// defined by this diff. TODO: do this.
    pub max_power_diff: u8,
}

impl Default for ArqStrat {
    fn default() -> Self {
        Self {
            min_coverage: 50.0,
            // this buffer implies min-max chunk count of 8-16
            buffer: 0.1425,
            min_power: 1,
            // the max power should be set so that a full arq is representable,
            // i.e. the number of chunks needed at max power is within the valid
            // range of chunk count
            max_power: 32 - 3,
            power_std_dev_threshold: 1.0,
            max_power_diff: 2,
        }
    }
}

impl ArqStrat {
    /// The midline between min and max coverage
    pub fn midline_coverage(&self) -> f64 {
        (self.min_coverage + self.max_coverage()) / 2.0
    }

    /// The max coverage as expressed by the min coverage and the buffer
    pub fn max_coverage(&self) -> f64 {
        self.min_coverage * (self.buffer + 1.0)
    }

    /// The width of the buffer range
    pub fn buffer_width(&self) -> f64 {
        self.min_coverage * self.buffer
    }

    /// The lower bound of number of chunks to maintain in an arq.
    /// When the chunk count falls below this number, halve the chunk size.
    pub fn min_chunks(&self) -> u32 {
        self.chunk_count_threshold().ceil() as u32
    }

    /// The upper bound of number of chunks to maintain in an arq.
    /// When the chunk count exceeds this number, double the chunk size.
    /// This is expressed in terms of min_chunks because we want this value
    /// to always be even.
    pub fn max_chunks(&self) -> u32 {
        let max_chunks = self.min_chunks() * 2;
        assert!(max_chunks % 2 == 0);
        max_chunks
    }

    /// The chunk count which gives us the quantization resolution appropriate
    /// for maintaining the buffer when adding/removing single chunks.
    /// Used in `min_chunks` and `max_chunks`.
    ///
    /// See this doc for rationale:
    /// https://hackmd.io/@hololtd/r1IAIbr5Y/https%3A%2F%2Fhackmd.io%2FK_fkBj6XQO2rCUZRRL9n2g
    fn chunk_count_threshold(&self) -> f64 {
        (self.buffer + 1.0) / self.buffer
    }
}
