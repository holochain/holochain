use crate::{CellT, ConductorApiResult};
use async_trait::async_trait;
use sx_types::agent::CellId;

#[async_trait]
pub trait ConductorT: Sized + Send + Sync {
    type Cell: CellT;

    fn cell_by_id(
        &self,
        cell_id: &CellId,
    ) -> ConductorApiResult<&Self::Cell>;
}
