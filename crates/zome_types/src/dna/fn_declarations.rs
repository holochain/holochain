//! File holding all the structs for handling function declarations defined in DNA.

use serde::{Deserialize, Serialize};

/// Represents the type declaration for zome function parameter
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash)]
pub struct FnParameter {
    /// What type is this parameter?
    #[serde(rename = "type")]
    pub parameter_type: String,

    /// What is the name of this parameter?
    pub name: String,
}

impl FnParameter {
    /// Construct a new FnParameter
    #[allow(dead_code)]
    pub fn new<S: Into<String>>(n: S, t: S) -> FnParameter {
        FnParameter {
            name: n.into(),
            parameter_type: t.into(),
        }
    }
}

/// Represents a zome function declaration
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash)]
pub struct FnDeclaration {
    /// The name of this fn declaration.
    #[serde(default)]
    pub name: String,

    /// Input parameters to function.
    pub inputs: Vec<FnParameter>,

    /// Outputs from the function.
    pub outputs: Vec<FnParameter>,
}

impl Default for FnDeclaration {
    /// Defaults for a "fn_declarations" object.
    fn default() -> Self {
        FnDeclaration {
            name: String::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }
}

impl FnDeclaration {
    /// Allow sane defaults for `FnDecrlaration::new()`.
    pub fn new() -> Self {
        Default::default()
    }
}

/// Represents a group of named functions in the Zomes's "traits" array
#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, Hash)]
pub struct TraitFns {
    /// "functions" array
    #[serde(default)]
    pub functions: Vec<String>,
}

impl TraitFns {
    /// TraitFns Constructor
    pub fn new() -> Self {
        Default::default()
    }
}

/// Represents an trait definition for bridging
#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, Hash)]
pub struct Trait {
    /// "functions" array
    #[serde(default)]
    pub functions: Vec<FnDeclaration>,
}

impl Trait {
    /// Trait Constructor
    pub fn new() -> Self {
        Default::default()
    }
}
