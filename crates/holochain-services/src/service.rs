pub trait Service {
    /// The CellIds which must be running for this service to function
    fn cell_ids(&self) -> HashSet<&CellId>;
}
