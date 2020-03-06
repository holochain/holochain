use crate::{dna::Dna, prelude::*};
use std::convert::TryFrom;

pub fn test_dna(uuid: &str) -> Dna {
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
