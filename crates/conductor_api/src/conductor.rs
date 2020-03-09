use crate::{ApiCellT, ConductorApiResult};
use async_trait::async_trait;
use sx_types::cell::CellId;

#[async_trait]
pub trait ConductorT: Sized + Send + Sync {
    type Cell: ApiCellT;

    fn cell_by_id(
        &self,
        cell_id: &CellId,
    ) -> ConductorApiResult<&Self::Cell>;
}
