use crate::error::ConductorApiResult;
use crate::interface::CellConductorInterfaceT;
use sx_types::agent::CellId;
use async_trait::async_trait;

#[async_trait]
pub trait ConductorT: Sized {
    type Interface: CellConductorInterfaceT;

    fn cell_by_id(&self, cell_id: &CellId) -> ConductorApiResult<& <Self::Interface as CellConductorInterfaceT>::Cell>;
}
