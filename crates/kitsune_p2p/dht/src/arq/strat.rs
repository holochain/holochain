pub struct ArqStrat {
    pub min_coverage: f64,
    pub buffer: f64,
}

impl ArqStrat {
    /// The max coverage as expressed by the min coverage and the buffer
    pub fn max_coverage(&self) -> f64 {
        self.min_coverage * (self.buffer + 1.0)
    }

    /// The width of the buffer range
    pub fn buffer_width(&self) -> f64 {
        self.min_coverage * self.buffer
    }
}
