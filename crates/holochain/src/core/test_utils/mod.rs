use sx_types::{agent::AgentId, cell::CellId, test_utils::test_dna, dna::Dna};

pub fn fake_cell_id(name: &str) -> CellId {
    (name.clone().into(), fake_agent_id(name)).into()
}

pub fn fake_dna(uuid: &str) -> Dna {
    test_dna(uuid)
}

pub fn fake_agent_id(name: &str) -> AgentId {
    AgentId::generate_fake(name)
}