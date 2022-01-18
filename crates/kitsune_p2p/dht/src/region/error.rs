#[derive(Debug)]
pub enum RegionDiffError {}

pub type RegionDiffResult<T> = Result<T, RegionDiffError>;
