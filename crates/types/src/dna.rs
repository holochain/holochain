//! dna is a library for working with holochain dna files/entries.
//!
//! It includes utilities for representing dna structures in memory,
//! as well as serializing and deserializing dna, mainly to json format.

pub mod bridges;
pub mod capabilities;
pub mod entry_types;
pub mod error;
pub mod fn_declarations;
pub mod traits;
pub mod wasm;
pub mod zome;

use crate::{
    dna::{
        bridges::Bridge,
        entry_types::EntryTypeDef,
        error::DnaError,
        fn_declarations::{FnDeclaration, TraitFns},
    },
    entry::entry_type::{AppEntryType, EntryType},
    prelude::{Address, *},
};
use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
};

/// serde helper, provides a default newly generated v4 uuid
fn zero_uuid() -> String {
    String::from("00000000-0000-0000-0000-000000000000")
}

/// TODO: consider a newtype for this
pub type DnaAddress = Address;

/// Represents the top-level holochain dna object.
#[derive(Serialize, Deserialize, Clone, Debug, SerializedBytes, SerializedBytesAddress)]
pub struct Dna {
    /// The top-level "name" of a holochain application.
    #[serde(default)]
    pub name: String,

    /// The top-level "description" of a holochain application.
    #[serde(default)]
    pub description: String,

    /// The semantic version of your holochain application.
    #[serde(default)]
    pub version: String,

    /// A unique identifier to distinguish your holochain application.
    #[serde(default = "zero_uuid")]
    pub uuid: String,

    /// Which version of the holochain dna spec does this represent?
    #[serde(default)]
    pub dna_spec_version: String,

    /// Any arbitrary application properties can be included in this object.
    pub properties: SerializedBytes,

    /// An array of zomes associated with your holochain application.
    #[serde(default)]
    pub zomes: BTreeMap<String, zome::Zome>,
}

impl Eq for Dna {}

impl Dna {
    /// Return a Zome
    pub fn get_zome(&self, zome_name: &str) -> Result<&zome::Zome, DnaError> {
        self.zomes
            .get(zome_name)
            .ok_or_else(|| DnaError::ZomeNotFound(format!("Zome '{}' not found", &zome_name,)))
    }

    /// Return a Zome's TraitFns from a Zome and a Trait name.
    pub fn get_trait<'a>(&'a self, zome: &'a zome::Zome, trait_name: &str) -> Option<&'a TraitFns> {
        zome.traits.get(trait_name)
    }

    /// Return a Zome Function declaration from a Zome name and Function name.
    pub fn get_function_with_zome_name(
        &self,
        zome_name: &str,
        fn_name: &str,
    ) -> Result<&FnDeclaration, DnaError> {
        let zome = self.get_zome(zome_name)?;

        zome.get_function(&fn_name).ok_or_else(|| {
            DnaError::ZomeFunctionNotFound(format!(
                "Zome function '{}' not found in Zome '{}'",
                &fn_name, &zome_name
            ))
        })
    }

    /// Find a Zome and return it's WASM bytecode
    pub fn get_wasm_from_zome_name<T: Into<String>>(&self, zome_name: T) -> Option<&wasm::DnaWasm> {
        let zome_name = zome_name.into();
        self.get_zome(&zome_name).ok().map(|ref zome| &zome.code)
    }

    /// Return a Zome's Trait functions from a Zome name and trait name.
    pub fn get_trait_fns_with_zome_name(
        &self,
        zome_name: &str,
        trait_name: &str,
    ) -> Result<&TraitFns, DnaError> {
        let zome = self.get_zome(zome_name)?;

        self.get_trait(zome, &trait_name).ok_or_else(|| {
            DnaError::TraitNotFound(format!(
                "Trait '{}' not found in Zome '{}'",
                &trait_name, &zome_name
            ))
        })
    }

    /// Return the name of the zome holding a specified app entry_type
    pub fn get_zome_name_for_app_entry_type(
        &self,
        app_entry_type: &AppEntryType,
    ) -> Option<String> {
        let entry_type_name = String::from(app_entry_type.to_owned());
        // pre-condition: must be a valid app entry_type name
        assert!(EntryType::has_valid_app_name(&entry_type_name));
        // Browse through the zomes
        for (zome_name, zome) in &self.zomes {
            for zome_entry_type_name in zome.entry_types.keys() {
                if *zome_entry_type_name
                    == EntryType::App(AppEntryType::from(entry_type_name.to_string()))
                {
                    return Some(zome_name.clone());
                }
            }
        }
        None
    }

    /// Return the entry_type definition of a specified app entry_type
    pub fn get_entry_type_def(&self, entry_type_name: &str) -> Option<&EntryTypeDef> {
        // pre-condition: must be a valid app entry_type name
        assert!(EntryType::has_valid_app_name(entry_type_name));
        // Browse through the zomes
        for zome in self.zomes.values() {
            for (zome_entry_type_name, entry_type_def) in &zome.entry_types {
                if *zome_entry_type_name
                    == EntryType::App(AppEntryType::from(entry_type_name.to_string()))
                {
                    return Some(entry_type_def);
                }
            }
        }
        None
    }

    /// List the required bridges.
    pub fn get_required_bridges(&self) -> Vec<Bridge> {
        self.zomes
            .values()
            .map(|zome| zome.get_required_bridges())
            .flatten()
            .collect()
    }

    /// Check that all the zomes in the DNA have code with the required callbacks
    /// TODO: Add more advanced checks that actually try and call required functions
    pub fn verify(&self) -> Result<(), DnaError> {
        let errors: Vec<DnaError> = self
            .zomes
            .iter()
            .map(|(zome_name, zome)| {
                // currently just check the zome has some code
                if zome.code.code().len() > 0 {
                    Ok(())
                } else {
                    Err(DnaError::EmptyZome(zome_name.clone()))
                }
            })
            .filter_map(|r| r.err())
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(DnaError::Invalid(format!("invalid DNA: {:?}", errors)))
        }
    }
}

impl Hash for Dna {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let s: Vec<u8> = UnsafeBytes::from(SerializedBytes::try_from(self).unwrap()).into();
        s.hash(state);
    }
}

impl PartialEq for Dna {
    fn eq(&self, other: &Dna) -> bool {
        // need to guarantee that PartialEq and Hash always agree
        SerializedBytes::try_from(self).unwrap() == SerializedBytes::try_from(other).unwrap()
    }
}

#[cfg(test)]
pub mod tests {
    use crate::{
        dna::{
            bridges::{Bridge, BridgePresence, BridgeReference},
            entry_types::{EntryTypeDef, Sharing},
            fn_declarations::{FnDeclaration, FnParameter, Trait},
            wasm::DnaWasm,
            zome::{Config, Zome},
            Dna,
        },
        entry::entry_type::{AppEntryType, EntryType},
        persistence::cas::content::Address,
        test_utils::{fake_dna, fake_zome},
    };
    use holochain_serialized_bytes::prelude::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_dna_get_zome() {
        let dna = fake_dna("a");
        let result = dna.get_zome("foo zome");
        assert_eq!(
            format!("{:?}", result),
            "Err(ZomeNotFound(\"Zome \\\'foo zome\\\' not found\"))"
        );
        let zome = dna.get_zome("test").unwrap();
        assert_eq!(zome.description, "test");
    }

    #[test]
    fn test_dna_get_trait() {
        let dna = fake_dna("a");
        let zome = dna.get_zome("test").unwrap();
        let result = dna.get_trait(zome, "foo trait");
        assert!(result.is_none());
        let cap = dna.get_trait(zome, "hc_public").unwrap();
        assert_eq!(format!("{:?}", cap), "TraitFns { functions: [\"test\"] }");
    }

    #[test]
    fn test_dna_get_trait_with_zome_name() {
        let dna = fake_dna("a");
        let result = dna.get_trait_fns_with_zome_name("foo zome", "foo trait");
        assert_eq!(
            format!("{:?}", result),
            "Err(ZomeNotFound(\"Zome \\\'foo zome\\\' not found\"))"
        );
        let result = dna.get_trait_fns_with_zome_name("test", "foo trait");
        assert_eq!(
            format!("{:?}", result),
            "Err(TraitNotFound(\"Trait \\\'foo trait\\\' not found in Zome \\\'test\\\'\"))"
        );
        let trait_fns = dna
            .get_trait_fns_with_zome_name("test", "hc_public")
            .unwrap();
        assert_eq!(
            format!("{:?}", trait_fns),
            "TraitFns { functions: [\"test\"] }"
        );
        let trait_fns = dna
            .get_trait_fns_with_zome_name("test", "hc_public")
            .unwrap();
        assert_eq!(
            format!("{:?}", trait_fns),
            "TraitFns { functions: [\"test\"] }"
        );
    }

    #[test]
    fn test_dna_get_function_with_zome_name() {
        let dna = fake_dna("a");
        let result = dna.get_function_with_zome_name("foo zome", "foo fun");
        assert_eq!(
            format!("{:?}", result),
            "Err(ZomeNotFound(\"Zome \\\'foo zome\\\' not found\"))"
        );
        let result = dna.get_function_with_zome_name("test", "foo fun");
        assert_eq!(format!("{:?}",result),"Err(ZomeFunctionNotFound(\"Zome function \\\'foo fun\\\' not found in Zome \\\'test\\\'\"))");
        let fun = dna.get_function_with_zome_name("test", "test").unwrap();
        assert_eq!(
            format!("{:?}", fun),
            "FnDeclaration { name: \"test\", inputs: [], outputs: [] }"
        );
    }

    #[test]
    fn test_dna_verify() {
        let dna = fake_dna("a");
        assert!(dna.verify().is_ok())
    }

    #[test]
    fn test_dna_verify_fail() {
        // should error because code is empty
        let dna = Dna {
            name: String::new(),
            description: String::new(),
            version: String::new(),
            dna_spec_version: String::new(),
            properties: SerializedBytes::try_from(()).unwrap(),
            uuid: String::new(),
            zomes: {
                let mut v = BTreeMap::new();
                v.insert(
                    String::from("my_zome"),
                    Zome {
                        description: String::new(),
                        entry_types: BTreeMap::new(),
                        fn_declarations: vec![],
                        traits: BTreeMap::new(),
                        code: DnaWasm::new_invalid(),
                        bridges: vec![],
                        config: Config::default(),
                    },
                );
                v
            },
        };
        assert!(dna.verify().is_err())
    }

    // static UNIT_UUID: &'static str = "00000000-0000-0000-0000-000000000000";

    #[test]
    fn get_entry_type_def_test() {
        let mut dna = fake_dna("get_entry_type_def_test");
        let mut zome = fake_zome();
        let entry_type = EntryType::App(AppEntryType::from("bar"));
        let entry_type_def = EntryTypeDef::new();

        zome.entry_types.insert(entry_type, entry_type_def.clone());
        dna.zomes.insert("zome".to_string(), zome);

        assert_eq!(None, dna.get_entry_type_def("foo"));
        assert_eq!(Some(&entry_type_def), dna.get_entry_type_def("bar"));
    }

    #[test]
    fn get_wasm_from_zome_name() {
        let dna = Dna {
            name: String::new(),
            description: String::new(),
            version: String::new(),
            dna_spec_version: String::new(),
            properties: SerializedBytes::try_from(()).unwrap(),
            uuid: String::new(),
            zomes: {
                let mut v = BTreeMap::new();
                v.insert(
                    String::from("test zome"),
                    Zome {
                        description: String::new(),
                        entry_types: BTreeMap::new(),
                        fn_declarations: vec![],
                        traits: BTreeMap::new(),
                        code: DnaWasm::from(vec![1, 2, 3]),
                        bridges: vec![],
                        config: Config::default(),
                    },
                );
                v
            },
        };
        let wasm = dna.get_wasm_from_zome_name("test zome").unwrap();
        assert_eq!(vec![1, 2, 3], *wasm.code());

        let fail = dna.get_wasm_from_zome_name("non existant zome");
        assert_eq!(None, fail);
    }

    #[test]
    fn test_get_zome_name_for_entry_type() {
        let dna = Dna {
            name: String::new(),
            description: String::new(),
            version: String::new(),
            dna_spec_version: String::new(),
            properties: SerializedBytes::try_from(()).unwrap(),
            uuid: String::new(),
            zomes: {
                let mut v = BTreeMap::new();
                v.insert(
                    String::from("test zome"),
                    Zome {
                        description: String::new(),
                        entry_types: {
                            let mut v = BTreeMap::new();
                            v.insert(
                                AppEntryType::from("test type").into(),
                                EntryTypeDef {
                                    sharing: Sharing::Public,
                                    linked_from: vec![],
                                    links_to: vec![],
                                    properties: SerializedBytes::try_from(()).unwrap(),
                                },
                            );
                            v
                        },
                        fn_declarations: vec![],
                        traits: BTreeMap::new(),
                        code: DnaWasm::from(vec![1, 2, 3]),
                        bridges: vec![],
                        config: Config::default(),
                    },
                );
                v
            },
        };

        assert_eq!(
            dna.get_zome_name_for_app_entry_type(&AppEntryType::from("test type"))
                .unwrap(),
            "test zome".to_string()
        );
        assert!(dna
            .get_zome_name_for_app_entry_type(&AppEntryType::from("non existant entry type"))
            .is_none());
    }

    #[test]
    fn test_get_required_bridges() {
        let dna = Dna {
            name: String::new(),
            description: String::new(),
            version: String::new(),
            dna_spec_version: String::new(),
            properties: SerializedBytes::try_from(()).unwrap(),
            uuid: String::new(),
            zomes: {
                let mut v = BTreeMap::new();
                v.insert(
                    String::from("test zome"),
                    Zome {
                        description: String::new(),
                        entry_types: {
                            let mut v = BTreeMap::new();
                            v.insert(
                                AppEntryType::from("test type").into(),
                                EntryTypeDef {
                                    sharing: Sharing::Public,
                                    linked_from: vec![],
                                    links_to: vec![],
                                    properties: SerializedBytes::try_from(()).unwrap(),
                                },
                            );
                            v
                        },
                        fn_declarations: vec![],
                        traits: BTreeMap::new(),
                        code: DnaWasm::from(vec![1, 2, 3]),
                        bridges: vec![
                            Bridge {
                                handle: String::from("Vault"),
                                presence: BridgePresence::Optional,
                                reference: BridgeReference::Trait {
                                    traits: {
                                        let mut v = BTreeMap::new();
                                        v.insert(
                                            String::from("persona_management"),
                                            Trait {
                                                functions: vec![FnDeclaration {
                                                    name: String::from("get_happs"),
                                                    inputs: vec![],
                                                    outputs: vec![FnParameter {
                                                        name: "happs".into(),
                                                        parameter_type: "json".into(),
                                                    }],
                                                }],
                                            },
                                        );
                                        v
                                    },
                                },
                            },
                            Bridge {
                                handle: String::from("HCHC"),
                                presence: BridgePresence::Required,
                                reference: BridgeReference::Trait {
                                    traits: {
                                        let mut v = BTreeMap::new();
                                        v.insert(
                                            String::from("happ_directory"),
                                            Trait {
                                                functions: vec![FnDeclaration {
                                                    name: String::from("get_happs"),
                                                    inputs: vec![],
                                                    outputs: vec![FnParameter {
                                                        name: "happs".into(),
                                                        parameter_type: "json".into(),
                                                    }],
                                                }],
                                            },
                                        );
                                        v
                                    },
                                },
                            },
                            Bridge {
                                handle: String::from("DPKI"),
                                presence: BridgePresence::Required,
                                reference: BridgeReference::Address {
                                    dna_address: Address::from("Qmabcdef1234567890"),
                                },
                            },
                        ],
                        config: Config::default(),
                    },
                );
                v
            },
        };
        assert_eq!(
            dna.get_required_bridges(),
            vec![
                Bridge {
                    presence: BridgePresence::Required,
                    handle: String::from("HCHC"),
                    reference: BridgeReference::Trait {
                        traits: maplit::btreemap! {
                            String::from("happ_directory") => Trait {
                                functions: vec![
                                    FnDeclaration {
                                        name: String::from("get_happs"),
                                        inputs: vec![],
                                        outputs: vec![FnParameter{
                                            name: String::from("happs"),
                                            parameter_type: String::from("json"),
                                        }],
                                    }
                                ]
                            }
                        }
                    },
                },
                Bridge {
                    presence: BridgePresence::Required,
                    handle: String::from("DPKI"),
                    reference: BridgeReference::Address {
                        dna_address: Address::from("Qmabcdef1234567890"),
                    }
                },
            ]
        );
    }
}
