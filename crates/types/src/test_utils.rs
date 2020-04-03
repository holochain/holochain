//! Some common testing helpers.

use crate::{agent::AgentId, cell::CellId, dna::Dna, prelude::*};
use std::convert::TryFrom;

/// A fixture example CellId for unit testing.
pub fn fake_cell_id(name: &str) -> CellId {
    (name.to_string().into(), fake_agent_id(name)).into()
}

/// A fixture example AgentId for unit testing.
pub fn fake_agent_id(name: &str) -> AgentId {
    AgentId::generate_fake(name)
}

/// A fixture example Dna for unit testing.
pub fn fake_dna(uuid: &str) -> Dna {
    let fixture = format!(
        r#"{{
                "name": "test",
                "description": "test",
                "version": "test",
                "uuid": "{}",
                "dna_spec_version": "2.0",
                "properties": {{
                    "test": "test"
                }},
                "zomes": {{
                    "test": {{
                        "description": "test",
                        "config": {{}},
                        "entry_types": {{
                            "test": {{
                                "description": "test",
                                "sharing": "public",
                                "links_to": [
                                    {{
                                        "target_type": "test",
                                        "link_type": "test"
                                    }}
                                ],
                                "linked_from": []
                            }}
                        }},
                        "traits": {{
                            "hc_public": {{
                                "functions": ["test"]
                            }}
                        }},
                        "fn_declarations": [
                            {{
                                "name": "test",
                                "inputs": [],
                                "outputs": []
                            }}
                        ],
                        "code": {{
                            "code": "AAECAw=="
                        }},
                        "bridges": []
                    }}
                }}
            }}"#,
        uuid
    );
    Dna::try_from(JsonString::from_json(&fixture)).unwrap()
}
