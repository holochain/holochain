//! dna is a library for working with holochain dna files/entries.
//!
//! It includes utilities for representing dna structures in memory,
//! as well as serializing and deserializing dna, mainly to json format.
//!
//! # Examples
//!
//! ```
//! use sx_types::dna::Dna;
//! use holochain_json_api::json::JsonString;
//! use std::convert::TryFrom;
//!
//! let name = String::from("My Holochain DNA");
//!
//! let mut dna = Dna::empty();
//! dna.name = name.clone();
//!
//! let json = JsonString::from(dna.clone());
//!
//! let dna2 = Dna::try_from(json).expect("could not restore DNA from JSON");
//! assert_eq!(name, dna2.name);
//! ```

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
    error::{SkunkError, SkunkResult},
    prelude::Address,
};
use holochain_json_api::{
    error::{JsonError, JsonResult},
    json::JsonString,
};
use crate::persistence::cas::content::{AddressableContent, Content};
use multihash;
use serde::{Deserialize, Serialize};
use serde_json::{self, json, Value};
use std::{
    collections::BTreeMap,
    convert::TryFrom,
    hash::{Hash, Hasher},
};

/// serde helper, provides a default empty object
fn empty_object() -> Value {
    json!({})
}

/// serde helper, provides a default newly generated v4 uuid
fn zero_uuid() -> String {
    String::from("00000000-0000-0000-0000-000000000000")
}

/// TODO: consider a newtype for this
pub type DnaAddress = Address;

/// Represents the top-level holochain dna object.
#[derive(Serialize, Deserialize, Clone, Debug, DefaultJson)]
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
    #[serde(default = "empty_object")]
    pub properties: Value,

    /// An array of zomes associated with your holochain application.
    #[serde(default)]
    pub zomes: BTreeMap<String, zome::Zome>,
}

impl AddressableContent for Dna {
    fn content(&self) -> Content {
        Content::from(self.to_owned())
    }

    fn try_from_content(content: &Content) -> JsonResult<Self> {
        Ok(Dna::try_from(content.to_owned())?)
    }
}

impl Eq for Dna {}

impl Dna {
    /// Create a new in-memory dna structure with some default values.
    ///
    /// # Examples
    ///
    /// ```
    /// use sx_types::dna::Dna;
    ///
    /// let dna = Dna::empty();
    /// assert_eq!("", dna.name);
    ///
    /// ```
    pub fn empty() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            version: String::new(),
            uuid: zero_uuid(),
            dna_spec_version: String::from("2.0"),
            properties: empty_object(),
            zomes: BTreeMap::new(),
        }
    }

    /// Generate a pretty-printed json string from an in-memory dna struct.
    ///
    /// # Examples
    ///
    /// ```
    /// use sx_types::dna::Dna;
    ///
    /// let dna = Dna::empty();
    /// println!("json: {}", dna.to_json_pretty().expect("DNA should serialize"));
    ///
    /// ```
    pub fn to_json_pretty(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

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

    pub fn multihash(&self) -> Result<Vec<u8>, SkunkError> {
        let s = String::from(JsonString::from(self.to_owned()));
        multihash::encode(multihash::Hash::SHA2256, &s.into_bytes())
            .map_err(|error| SkunkError::new(error.to_string()))
    }

    pub fn get_required_bridges(&self) -> Vec<Bridge> {
        self.zomes
            .values()
            .map(|zome| zome.get_required_bridges())
            .flatten()
            .collect()
    }

    // Check that all the zomes in the DNA have code with the required callbacks
    // TODO: Add more advanced checks that actually try and call required functions
    pub fn verify(&self) -> SkunkResult<()> {
        let errors: Vec<SkunkError> = self
            .zomes
            .iter()
            .map(|(zome_name, zome)| {
                // currently just check the zome has some code
                if zome.code.code.len() > 0 {
                    Ok(())
                } else {
                    Err(SkunkError::new(format!("Zome {} has no code!", zome_name)))
                }
            })
            .filter_map(|r| r.err())
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(SkunkError::new(format!("invalid DNA: {:?}", errors)))
        }
    }
}

impl Hash for Dna {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let s = String::from(JsonString::from(self.to_owned()));
        s.hash(state);
    }
}

impl PartialEq for Dna {
    fn eq(&self, other: &Dna) -> bool {
        // need to guarantee that PartialEq and Hash always agree
        JsonString::from(self.to_owned()) == JsonString::from(other.to_owned())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    extern crate base64;
    use crate::{
        dna::{
            bridges::{Bridge, BridgePresence, BridgeReference},
            entry_types::EntryTypeDef,
            fn_declarations::{FnDeclaration, FnParameter, Trait},
            zome::tests::test_zome,
        },
        entry::entry_type::{AppEntryType, EntryType},
        test_utils::test_dna,
    };
    use holochain_json_api::json::JsonString;
    use crate::persistence::cas::content::Address;
    use std::convert::TryFrom;

    #[test]
    fn test_dna_new() {
        let dna = Dna::empty();
        assert_eq!(format!("{:?}",dna),"Dna { name: \"\", description: \"\", version: \"\", uuid: \"00000000-0000-0000-0000-000000000000\", dna_spec_version: \"2.0\", properties: Object({}), zomes: {} }")
    }

    #[test]
    fn test_dna_to_json_pretty() {
        let dna = Dna::empty();
        assert_eq!(format!("{:?}",dna.to_json_pretty()),"Ok(\"{\\n  \\\"name\\\": \\\"\\\",\\n  \\\"description\\\": \\\"\\\",\\n  \\\"version\\\": \\\"\\\",\\n  \\\"uuid\\\": \\\"00000000-0000-0000-0000-000000000000\\\",\\n  \\\"dna_spec_version\\\": \\\"2.0\\\",\\n  \\\"properties\\\": {},\\n  \\\"zomes\\\": {}\\n}\")")
    }

    #[test]
    fn test_dna_get_zome() {
        let dna = test_dna("a");
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
        let dna = test_dna("a");
        let zome = dna.get_zome("test").unwrap();
        let result = dna.get_trait(zome, "foo trait");
        assert!(result.is_none());
        let cap = dna.get_trait(zome, "hc_public").unwrap();
        assert_eq!(format!("{:?}", cap), "TraitFns { functions: [\"test\"] }");
    }

    #[test]
    fn test_dna_get_trait_with_zome_name() {
        let dna = test_dna("a");
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
        let dna = test_dna("a");
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
        let dna = test_dna("a");
        assert!(dna.verify().is_ok())
    }

    #[test]
    fn test_dna_verify_fail() {
        // should error because code is empty
        let dna = Dna::try_from(JsonString::from_json(
            r#"{
                "zomes": {
                    "my_zome": {
                        "code": {"code": ""}
                    }
                }
            }"#,
        ))
        .unwrap();
        assert!(dna.verify().is_err())
    }

    static UNIT_UUID: &'static str = "00000000-0000-0000-0000-000000000000";

    fn test_empty_dna() -> Dna {
        Dna::empty()
    }

    #[test]
    fn get_entry_type_def_test() {
        let mut dna = test_empty_dna();
        let mut zome = test_zome();
        let entry_type = EntryType::App(AppEntryType::from("bar"));
        let entry_type_def = EntryTypeDef::new();

        zome.entry_types
            .insert(entry_type.into(), entry_type_def.clone());
        dna.zomes.insert("zome".to_string(), zome);

        assert_eq!(None, dna.get_entry_type_def("foo"));
        assert_eq!(Some(&entry_type_def), dna.get_entry_type_def("bar"));
    }

    #[test]
    fn can_parse_and_output_json() {
        let dna = test_empty_dna();

        let serialized = serde_json::to_string(&dna).unwrap();

        let deserialized: Dna = serde_json::from_str(&serialized).unwrap();

        assert_eq!(String::from("2.0"), deserialized.dna_spec_version);
    }

    #[test]
    fn can_parse_and_output_json_helpers() {
        let dna = test_empty_dna();

        let json_string = JsonString::from(dna);

        let deserialized = Dna::try_from(json_string).unwrap();

        assert_eq!(String::from("2.0"), deserialized.dna_spec_version);
    }

    #[test]
    fn parse_and_serialize_compare() {
        let fixture = String::from(
            r#"{
                "name": "test",
                "description": "test",
                "version": "test",
                "uuid": "00000000-0000-0000-0000-000000000000",
                "dna_spec_version": "2.0",
                "properties": {
                    "test": "test"
                },
                "zomes": {
                    "test": {
                        "description": "test",
                        "config": {},
                        "entry_types": {
                            "test": {
                                "properties": "test",
                                "sharing": "public",
                                "links_to": [
                                    {
                                        "target_type": "test",
                                        "link_type": "test"
                                    }
                                ],
                                "linked_from": []
                            }
                        },
                        "traits": {
                            "hc_public": {
                                "functions": ["test"]
                            }
                        },
                        "fn_declarations": [
                            {
                                "name": "test",
                                "inputs": [],
                                "outputs": []
                            }
                        ],
                        "code": {
                            "code": "AAECAw=="
                        },
                        "bridges": []
                    }
                }
            }"#,
        )
        .replace(char::is_whitespace, "");

        let dna = Dna::try_from(JsonString::from_json(&fixture.clone())).unwrap();

        println!("{}", dna.to_json_pretty().unwrap());

        let serialized = String::from(JsonString::from(dna)).replace(char::is_whitespace, "");

        assert_eq!(fixture, serialized);
    }

    #[test]
    fn default_value_test() {
        let mut dna = Dna::empty();
        dna.uuid = String::from(UNIT_UUID);

        let mut zome = zome::Zome::empty();
        zome.entry_types
            .insert("".into(), entry_types::EntryTypeDef::new());
        dna.zomes.insert("".to_string(), zome);

        let expected = JsonString::from(dna.clone());
        println!("{:?}", expected);

        let fixture = Dna::try_from(JsonString::from_json(
            r#"{
                "name": "",
                "description": "",
                "version": "",
                "uuid": "00000000-0000-0000-0000-000000000000",
                "dna_spec_version": "2.0",
                "properties": {},
                "zomes": {
                    "": {
                        "description": "",
                        "config": {},
                        "entry_types": {
                            "": {
                                "description": "",
                                "sharing": "public",
                                "links_to": [],
                                "linked_from": []
                            }
                        },
                        "traits": {},
                        "fn_declarations": [],
                        "code": {"code": ""}
                    }
                }
            }"#,
        ))
        .unwrap();

        assert_eq!(dna, fixture);
    }

    #[test]
    fn parse_with_defaults_dna() {
        let dna = Dna::try_from(JsonString::from_json(
            r#"{
            }"#,
        ))
        .unwrap();

        assert!(dna.uuid.len() > 0);
    }

    #[test]
    fn parse_with_defaults_entry_type() {
        let dna = Dna::try_from(JsonString::from_json(
            r#"{
                "zomes": {
                    "zome1": {
                        "code": {
                            "code": ""
                        },
                        "entry_types": {
                            "type1": {}
                        }
                    }
                }
            }"#,
        ))
        .unwrap();

        assert_eq!(
            dna.zomes
                .get("zome1")
                .unwrap()
                .entry_types
                .get(&"type1".into())
                .unwrap()
                .sharing,
            entry_types::Sharing::Public
        );
    }

    #[test]
    fn parse_wasm() {
        let dna = Dna::try_from(JsonString::from_json(
            r#"{
                "zomes": {
                    "zome1": {
                        "entry_types": {
                            "type1": {}
                        },
                        "code": {
                            "code": "AAECAw=="
                        }
                    }
                }
            }"#,
        ))
        .unwrap();

        assert_eq!(vec![0, 1, 2, 3], *dna.zomes.get("zome1").unwrap().code.code);
    }

    #[test]
    #[should_panic]
    fn parse_fail_if_bad_type_dna() {
        Dna::try_from(JsonString::from_json(
            r#"{
                "name": 42
            }"#,
        ))
        .unwrap();
    }

    #[test]
    #[should_panic]
    fn parse_fail_if_bad_type_zome() {
        Dna::try_from(JsonString::from_json(
            r#"{
                "zomes": {
                    "zome1": {
                        "description": 42
                    }
                }
            }"#,
        ))
        .unwrap();
    }

    #[test]
    #[should_panic]
    fn parse_fail_if_bad_type_entry_type() {
        Dna::try_from(JsonString::from_json(
            r#"{
                "zomes": {
                    "zome1": {
                        "entry_types": {
                            "test": {
                                "properties": 42
                            }
                        }
                    }
                }
            }"#,
        ))
        .unwrap();
    }

    #[test]
    fn parse_accepts_arbitrary_dna_properties() {
        let dna = Dna::try_from(JsonString::from_json(
            r#"{
                "properties": {
                    "str": "hello",
                    "num": 3.14159,
                    "bool": true,
                    "null": null,
                    "arr": [1, 2],
                    "obj": {"a": 1, "b": 2}
                }
            }"#,
        ))
        .unwrap();

        let props = dna.properties.as_object().unwrap();

        assert_eq!("hello", props.get("str").unwrap().as_str().unwrap());
        assert_eq!(3.14159, props.get("num").unwrap().as_f64().unwrap());
        assert_eq!(true, props.get("bool").unwrap().as_bool().unwrap());
        assert!(props.get("null").unwrap().is_null());
        assert_eq!(
            1_i64,
            props.get("arr").unwrap().as_array().unwrap()[0]
                .as_i64()
                .unwrap()
        );
        assert_eq!(
            1_i64,
            props
                .get("obj")
                .unwrap()
                .as_object()
                .unwrap()
                .get("a")
                .unwrap()
                .as_i64()
                .unwrap()
        );
    }

    #[test]
    fn get_wasm_from_zome_name() {
        let dna = Dna::try_from(JsonString::from_json(
            r#"{
                "name": "test",
                "description": "test",
                "version": "test",
                "uuid": "00000000-0000-0000-0000-000000000000",
                "dna_spec_version": "2.0",
                "properties": {
                    "test": "test"
                },
                "zomes": {
                    "test zome": {
                        "name": "test zome",
                        "description": "test",
                        "config": {},
                        "entry_types": {},
                        "traits": {
                            "hc_public": {
                            }
                        },
                        "fn_declarations": [],
                        "code": {
                            "code": "AAECAw=="
                        }
                    }
                }
            }"#,
        ))
        .unwrap();

        let wasm = dna.get_wasm_from_zome_name("test zome");
        assert_eq!("AAECAw==", base64::encode(&*wasm.unwrap().code));

        let fail = dna.get_wasm_from_zome_name("non existant zome");
        assert_eq!(None, fail);
    }

    #[test]
    fn test_get_zome_name_for_entry_type() {
        let dna = Dna::try_from(JsonString::from_json(
            r#"{
                "name": "test",
                "description": "test",
                "version": "test",
                "uuid": "00000000-0000-0000-0000-000000000000",
                "dna_spec_version": "2.0",
                "properties": {
                    "test": "test"
                },
                "zomes": {
                    "test zome": {
                        "name": "test zome",
                        "description": "test",
                        "config": {},
                        "traits": {
                            "hc_public": {
                                "functions": []
                            }
                        },
                        "fn_declarations": [],
                        "entry_types": {
                            "test type": {
                                "description": "",
                                "sharing": "public"
                            }
                        },
                        "code": {
                            "code": ""
                        }
                    }
                }
            }"#,
        ))
        .unwrap();

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
        let dna = Dna::try_from(JsonString::from_json(
            r#"{
                "name": "test",
                "description": "test",
                "version": "test",
                "uuid": "00000000-0000-0000-0000-000000000000",
                "dna_spec_version": "2.0",
                "properties": {
                    "test": "test"
                },
                "zomes": {
                    "test zome": {
                        "name": "test zome",
                        "description": "test",
                        "config": {},
                        "traits": {
                            "hc_public": {
                                "functions": []
                            }
                        },
                        "fn_declarations": [],
                        "entry_types": {
                            "test type": {
                                "description": "",
                                "sharing": "public"
                            }
                        },
                        "code": {
                            "code": ""
                        },
                        "bridges": [
                            {
                                "presence": "required",
                                "handle": "DPKI",
                                "reference": {
                                    "dna_address": "Qmabcdef1234567890"
                                }
                            },
                            {
                                "presence": "optional",
                                "handle": "Vault",
                                "reference": {
                                    "traits": {
                                        "persona_management": {
                                            "functions": [
                                                {
                                                    "name": "get_persona",
                                                    "inputs": [{"name": "domain", "type": "string"}],
                                                    "outputs": [{"name": "persona", "type": "json"}]
                                                }
                                            ]
                                        }
                                    }
                                }
                            },
                            {
                                "presence": "required",
                                "handle": "HCHC",
                                "reference": {
                                    "traits": {
                                        "happ_directory": {
                                            "functions": [
                                                {
                                                    "name": "get_happs",
                                                    "inputs": [],
                                                    "outputs": [{"name": "happs", "type": "json"}]
                                                }
                                            ]
                                        }
                                    }
                                }
                            }
                        ]
                    }
                }
            }"#,
        ))
        .unwrap();

        assert_eq!(
            dna.get_required_bridges(),
            vec![
                Bridge {
                    presence: BridgePresence::Required,
                    handle: String::from("DPKI"),
                    reference: BridgeReference::Address {
                        dna_address: Address::from("Qmabcdef1234567890"),
                    }
                },
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
            ]
        );
    }
}
