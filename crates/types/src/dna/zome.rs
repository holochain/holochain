//! sx_types::dna::zome is a set of structs for working with holochain dna.

use crate::{
    dna::{
        bridges::{Bridge, BridgePresence},
        entry_types::{self, deserialize_entry_types, serialize_entry_types, EntryTypeDef},
        fn_declarations::{FnDeclaration, FnParameter, TraitFns},
        traits::ReservedTraitNames,
        wasm::DnaWasm,
    },
    entry::entry_type::EntryType,
};
use holochain_serialized_bytes::prelude::*;
use std::collections::BTreeMap;

/// Represents the "config" object on a "zome".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash)]
pub struct Config {}

impl Default for Config {
    /// Provide defaults for the "zome" "config" object.
    fn default() -> Self {
        Config {}
    }
}

impl Config {
    /// Allow sane defaults for `Config::new()`.
    pub fn new() -> Self {
        Default::default()
    }
}

/// Map of EntryType to EntryTypeDef
pub type ZomeEntryTypes = BTreeMap<EntryType, EntryTypeDef>;

/// Map of String to Trait Functions.
pub type ZomeTraits = BTreeMap<String, TraitFns>;

/// List of Function Declarations.
pub type ZomeFnDeclarations = Vec<FnDeclaration>;

/// Represents an individual "zome".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, SerializedBytes)]
pub struct Zome {
    /// A description of this zome.
    #[serde(default)]
    pub description: String,

    /// Configuration associated with this zome.
    /// Note, this should perhaps be a more free-form serde_json::Value,
    /// "throw-errors" may not make sense for wasm, or other ribosome types.
    #[serde(default)]
    pub config: Config,

    /// An array of entry_types associated with this zome.
    #[serde(default)]
    #[serde(serialize_with = "serialize_entry_types")]
    #[serde(deserialize_with = "deserialize_entry_types")]
    pub entry_types: ZomeEntryTypes,

    /// An array of traits defined in this zome.
    #[serde(default)]
    pub traits: ZomeTraits,

    /// An array of functions declared in this this zome.
    #[serde(default)]
    pub fn_declarations: ZomeFnDeclarations,

    /// Validation code for this entry_type.
    pub code: DnaWasm,

    /// A list of bridges to other DNAs that this DNA can use or depends on.
    #[serde(default)]
    pub bridges: Vec<Bridge>,
}

impl Eq for Zome {}

impl Zome {
    /// Allow sane defaults for `Zome::new()`.
    pub fn new(
        description: &str,
        config: &Config,
        entry_types: &BTreeMap<EntryType, entry_types::EntryTypeDef>,
        fn_declarations: &[FnDeclaration],
        traits: &BTreeMap<String, TraitFns>,
        code: &DnaWasm,
    ) -> Zome {
        Zome {
            description: description.into(),
            config: config.clone(),
            entry_types: entry_types.to_owned(),
            fn_declarations: fn_declarations.to_owned(),
            traits: traits.to_owned(),
            code: code.clone(),
            bridges: Vec::new(),
        }
    }

    /// List the required bridges for this Zome.
    pub fn get_required_bridges(&self) -> Vec<Bridge> {
        self.bridges
            .iter()
            .filter(|bridge| bridge.presence == BridgePresence::Required)
            .cloned()
            .collect()
    }

    /// Add a function declaration to a Zome
    pub fn add_fn_declaration(
        &mut self,
        name: String,
        inputs: Vec<FnParameter>,
        outputs: Vec<FnParameter>,
    ) {
        self.fn_declarations.push(FnDeclaration {
            name,
            inputs,
            outputs,
        });
    }

    /// Return a Function declaration from a Zome
    pub fn get_function(&self, fn_name: &str) -> Option<&FnDeclaration> {
        self.fn_declarations
            .iter()
            .find(|ref fn_decl| fn_decl.name == fn_name)
    }

    /// Helper function for finding out if a given function call is public
    pub fn is_fn_public(&self, fn_name: &str) -> bool {
        let pub_trait = ReservedTraitNames::Public.as_str();
        self.traits.iter().any(|(trait_name, trait_fns)| {
            trait_name == pub_trait && trait_fns.functions.contains(&fn_name.to_owned())
        })
    }
}

#[cfg(test)]
pub mod tests {
    use crate::{
        dna::fn_declarations::{FnDeclaration, FnParameter},
        test_utils::fake_zome,
    };

    #[test]
    fn test_zome_add_fn_declaration() {
        let base = {
            let mut zome = fake_zome();
            zome.fn_declarations = vec![];
            zome
        };

        assert_eq!(base.fn_declarations.len(), 0);
        let mut actual = base.clone();
        actual.add_fn_declaration(
            String::from("hello"),
            vec![],
            vec![FnParameter {
                name: String::from("greeting"),
                parameter_type: String::from("String"),
            }],
        );
        assert_eq!(actual.fn_declarations.len(), 1);

        let mut expected = base;
        expected.fn_declarations.push(FnDeclaration {
            name: String::from("hello"),
            inputs: vec![],
            outputs: vec![FnParameter {
                name: String::from("greeting"),
                parameter_type: String::from("String"),
            }],
        });
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_zome_get_function() {
        let mut zome = fake_zome();
        zome.add_fn_declaration(String::from("test"), vec![], vec![]);
        let result = zome.get_function("foo func");
        assert!(result.is_none());
        let fun = zome.get_function("test").unwrap();
        assert_eq!(
            format!("{:?}", fun),
            "FnDeclaration { name: \"test\", inputs: [], outputs: [] }"
        );
    }
}
