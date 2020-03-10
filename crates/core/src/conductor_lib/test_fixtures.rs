use test_utils::create_test_dna_with_wat;
use sx_types::dna::fn_declarations::FnDeclaration;
use crate::config::DnaLoader;
use std::convert::TryFrom;
use std::{path::PathBuf, sync::Arc};
use sx_types::dna::{Dna, bridges::Bridge};
use sx_types::error::SkunkError;
use sx_types::prelude::*;

pub fn test_dna_loader() -> DnaLoader {
    let loader = Box::new(|path: &PathBuf| {
        Ok(match path.to_str().unwrap().as_ref() {
            "bridge/callee.dna" => callee_dna(),
            "bridge/caller.dna" => caller_dna(),
            "bridge/caller_dna_ref.dna" => caller_dna_with_dna_reference(),
            "bridge/caller_bogus_trait_ref.dna" => caller_dna_with_bogus_trait_reference(),
            "bridge/caller_without_required.dna" => caller_dna_without_required(),
            _ => Dna::try_from(JsonString::from_json(&example_dna_string())).unwrap(),
        })
    }) as Box<dyn FnMut(&PathBuf) -> Result<Dna, SkunkError> + Send + Sync>;
    Arc::new(loader)
}

pub fn example_dna_string() -> String {
    r#"{
            "name": "my dna",
            "description": "",
            "version": "",
            "uuid": "00000000-0000-0000-0000-000000000001",
            "dna_spec_version": "2.0",
            "properties": {},
            "zomes": {
                "": {
                    "description": "",
                    "config": {},
                    "entry_types": {
                        "": {
                            "description": "",
                            "sharing": "public"
                        }
                    },
                    "traits": {
                        "test": {
                            "functions": ["test"]
                         }
                    },
                    "fn_declarations": [
                        {
                            "name": "test",
                            "inputs": [
                                {
                                    "name": "post",
                                    "type": "string"
                                }
                            ],
                            "outputs" : [
                                {
                                    "name": "hash",
                                    "type": "string"
                                }
                            ]
                        }
                    ],
                    "code": {
                        "code": "AAECAw=="
                    },
                    "bridges": [
                        {
                            "presence": "optional",
                            "handle": "my favourite instance!",
                            "reference": {
                                "traits": {}
                            }
                        }
                    ]
                }
            }
        }"#
    .to_string()
}

pub fn callee_wat() -> String {
    r#"
(module

(memory 1)
(export "memory" (memory 0))

(func
    (export "__hdk_validate_app_entry")
    (param $allocation i64)
    (result i64)

    (i64.const 0)
)

(func
    (export "__hdk_validate_agent_entry")
    (param $allocation i64)
    (result i64)

    (i64.const 0)
)

(func
    (export "__hdk_validate_link")
    (param $allocation i64)
    (result i64)

    (i64.const 0)
)


(func
    (export "__hdk_get_validation_package_for_entry_type")
    (param $allocation i64)
    (result i64)

    ;; This writes "Entry" into memory
    (i64.store (i32.const 0) (i64.const 34))
    (i64.store (i32.const 1) (i64.const 69))
    (i64.store (i32.const 2) (i64.const 110))
    (i64.store (i32.const 3) (i64.const 116))
    (i64.store (i32.const 4) (i64.const 114))
    (i64.store (i32.const 5) (i64.const 121))
    (i64.store (i32.const 6) (i64.const 34))

    (i64.const 7)
)

(func
    (export "__hdk_get_validation_package_for_link")
    (param $allocation i64)
    (result i64)

    ;; This writes "Entry" into memory
    (i64.store (i32.const 0) (i64.const 34))
    (i64.store (i32.const 1) (i64.const 69))
    (i64.store (i32.const 2) (i64.const 110))
    (i64.store (i32.const 3) (i64.const 116))
    (i64.store (i32.const 4) (i64.const 114))
    (i64.store (i32.const 5) (i64.const 121))
    (i64.store (i32.const 6) (i64.const 34))

    (i64.const 7)
)

(func
    (export "__list_traits")
    (param $allocation i64)
    (result i64)

    (i64.const 0)
)

(func
    (export "__list_functions")
    (param $allocation i64)
    (result i64)

    (i64.const 0)
)

(func
    (export "hello")
    (param $allocation i64)
    (result i64)

    ;; This writes "Holo World" into memory
    (i64.store (i32.const 0) (i64.const 72))
    (i64.store (i32.const 1) (i64.const 111))
    (i64.store (i32.const 2) (i64.const 108))
    (i64.store (i32.const 3) (i64.const 111))
    (i64.store (i32.const 4) (i64.const 32))
    (i64.store (i32.const 5) (i64.const 87))
    (i64.store (i32.const 6) (i64.const 111))
    (i64.store (i32.const 7) (i64.const 114))
    (i64.store (i32.const 8) (i64.const 108))
    (i64.store (i32.const 9) (i64.const 100))

    (i64.const 10)
)
)
            "#
    .to_string()
}

fn bridge_call_fn_declaration() -> FnDeclaration {
    FnDeclaration {
        name: String::from("hello"),
        inputs: vec![],
        outputs: vec![dna::fn_declarations::FnParameter {
            name: String::from("greeting"),
            parameter_type: String::from("String"),
        }],
    }
}

fn callee_dna() -> Dna {
    let wat = &callee_wat();
    let mut dna = create_test_dna_with_wat("greeter", Some(wat));
    dna.uuid = String::from("basic_bridge_call");
    let fn_declaration = bridge_call_fn_declaration();

    {
        let zome = dna.zomes.get_mut("greeter").unwrap();
        zome.fn_declarations.push(fn_declaration.clone());
        zome.traits
            .get_mut("hc_public")
            .unwrap()
            .functions
            .push(fn_declaration.name.clone());
        zome.traits.insert(
            String::from("greetable"),
            TraitFns {
                functions: vec![fn_declaration.name.clone()],
            },
        );
    }

    dna
}

fn caller_dna() -> Dna {
    let mut path = PathBuf::new();

    path.push(wasm_target_dir(
        // FIXME This should change to core
        &String::from("conductor_lib").into(),
        &String::from("test-bridge-caller").into(),
    ));
    let wasm_path_component: PathBuf = [
        String::from("wasm32-unknown-unknown"),
        String::from("release"),
        String::from("test_bridge_caller.wasm"),
    ]
    .iter()
    .collect();
    path.push(wasm_path_component);

    let wasm = create_wasm_from_file(&path);
    let defs = create_test_defs_with_fn_names(vec![
        "call_bridge".to_string(),
        "call_bridge_error".to_string(),
    ]);
    let mut dna = create_test_dna_with_defs("test_zome", defs, &wasm);
    dna.uuid = String::from("basic_bridge_call");
    {
        let zome = dna.zomes.get_mut("test_zome").unwrap();
        zome.bridges.push(Bridge {
            presence: BridgePresence::Required,
            handle: String::from("test-callee"),
            reference: BridgeReference::Trait {
                traits: btreemap! {
                    String::from("greetable") => Trait{
                        functions: vec![bridge_call_fn_declaration()]
                    }
                },
            },
        });
        zome.bridges.push(Bridge {
            presence: BridgePresence::Optional,
            handle: String::from("DPKI"),
            reference: BridgeReference::Trait {
                traits: BTreeMap::new(),
            },
        });
        zome.bridges.push(Bridge {
            presence: BridgePresence::Optional,
            handle: String::from("happ-store"),
            reference: BridgeReference::Trait {
                traits: BTreeMap::new(),
            },
        });
    }

    dna
}

fn caller_dna_with_dna_reference() -> Dna {
    let mut dna = caller_dna();
    {
        let bridge = dna
            .zomes
            .get_mut("test_zome")
            .unwrap()
            .bridges
            .get_mut(0)
            .unwrap();
        bridge.reference = BridgeReference::Address {
            dna_address: Address::from("fake bridge reference"),
        };
    }
    dna
}

fn caller_dna_with_bogus_trait_reference() -> Dna {
    let mut dna = caller_dna();
    {
        let bridge = dna
            .zomes
            .get_mut("test_zome")
            .unwrap()
            .bridges
            .get_mut(0)
            .unwrap();
        let mut fn_declaration = bridge_call_fn_declaration();
        fn_declaration
            .inputs
            .push(dna::fn_declarations::FnParameter {
                name: String::from("additional_parameter"),
                parameter_type: String::from("String"),
            });
        bridge.reference = BridgeReference::Trait {
            traits: btreemap! {
                String::from("greetable") => Trait{
                    functions: vec![fn_declaration]
                }
            },
        };
    }
    dna
}

fn caller_dna_without_required() -> Dna {
    let mut dna = caller_dna();
    {
        let bridge = dna
            .zomes
            .get_mut("test_zome")
            .unwrap()
            .bridges
            .get_mut(0)
            .unwrap();
        bridge.presence = BridgePresence::Optional;
        bridge.reference = BridgeReference::Trait {
            traits: BTreeMap::new(),
        };
    }
    dna
}

pub fn bridge_dna_ref_test_toml(caller_dna: &str, callee_dna: &str) -> String {
    format!(
        r#"
[[agents]]
id = "test-agent-1"
name = "Holo Tester 1"
public_address = "{}"
keystore_file = "holo_tester1.key"

[[dnas]]
id = "bridge-callee"
file = "{}"
hash = "Qm328wyq38924y"

[[dnas]]
id = "bridge-caller"
file = "{}"
hash = "Qm328wyq38924y"

[[instances]]
id = "bridge-callee"
dna = "bridge-callee"
agent = "test-agent-1"
[instances.storage]
type = "memory"

[[instances]]
id = "bridge-caller"
dna = "bridge-caller"
agent = "test-agent-1"
[instances.storage]
type = "memory"

[[bridges]]
caller_id = "bridge-caller"
callee_id = "bridge-callee"
handle = "test-callee"
"#,
        test_keybundle(1).get_id(),
        callee_dna,
        caller_dna,
    )
}
