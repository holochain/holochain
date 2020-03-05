use crate::cell::CellId;
use sx_types::{agent::AgentId, prelude::*};

pub fn fake_cell_id(name: &str) -> CellId {
    (name.clone().into(), AgentId::generate_fake(name)).into()
}
