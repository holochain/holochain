use crate::{CellConductorInterfaceT, ConductorApiResult};
use async_trait::async_trait;
use sx_types::agent::CellId;

#[async_trait]
pub trait ConductorT: Sized + Send + Sync {
    type Interface: CellConductorInterfaceT;

    fn cell_by_id(
        &self,
        cell_id: &CellId,
    ) -> ConductorApiResult<&<Self::Interface as CellConductorInterfaceT>::Cell>;
}
