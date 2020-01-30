//! File holding all the structs for handling function declarations defined in DNA.

/// Represents the type declaration for zome function parameter
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash)]
pub struct FnParameter {
    #[serde(rename = "type")]
    pub parameter_type: String,
    pub name: String,
}

impl FnParameter {
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
    pub inputs: Vec<FnParameter>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_fns_build_and_compare() {
        let fixture: TraitFns = serde_json::from_str(
            r#"{
                "functions": ["test"]
            }"#,
        )
        .unwrap();

        let mut trait_fns = TraitFns::new();
        trait_fns.functions.push(String::from("test"));
        assert_eq!(fixture, trait_fns);
    }

    #[test]
    fn test_trait_build_and_compare() {
        let fixture: Trait = serde_json::from_str(
            r#"{
                "functions": [
                    {
                        "name": "test",
                        "inputs" : [
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
                ]
            }"#,
        )
        .unwrap();

        let mut trt = Trait::new();
        let mut fn_dec = FnDeclaration::new();
        fn_dec.name = String::from("test");
        let input = FnParameter::new("post", "string");
        let output = FnParameter::new("hash", "string");
        fn_dec.inputs.push(input);
        fn_dec.outputs.push(output);
        trt.functions.push(fn_dec);

        assert_eq!(fixture, trt);
    }
}
