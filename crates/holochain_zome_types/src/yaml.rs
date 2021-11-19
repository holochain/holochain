use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
struct Mapping(HashMap<Value, Value>);

/// Adapted from serde_yaml::Mapping.
#[allow(clippy::derive_hash_xor_eq)]
impl Hash for Mapping {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash the kv pairs in a way that is not sensitive to their order.
        let mut xor = 0;
        for (k, v) in self.0.iter() {
            let mut hasher = DefaultHasher::new();
            k.hash(&mut hasher);
            v.hash(&mut hasher);
            xor ^= hasher.finish();
        }
        xor.hash(state);
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
enum Number {
    PosInt(u64),
    /// Always less than zero.
    NegInt(i64),
    /// May be infinite or NaN.
    Float(f64),
}

/// Adapted from serde_yaml::Mapping.
#[allow(clippy::derive_hash_xor_eq)]
impl Hash for Number {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Number::Float(f) => {
                // Something extra to avoid a collision with u64 in PosInt.
                1.hash(state);
                f.to_bits().hash(state);
            }
            Number::PosInt(u) => u.hash(state),
            Number::NegInt(i) => i.hash(state),
        }
    }
}

impl PartialEq for Number {
    fn eq(&self, other: &Number) -> bool {
        match (self, other) {
            (Number::PosInt(a), Number::PosInt(b)) => a == b,
            (Number::NegInt(a), Number::NegInt(b)) => a == b,
            (Number::Float(a), Number::Float(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Number {}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Eq, PartialEq, Hash)]
pub enum Value {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Sequence(Vec<Value>),
    Mapping(Mapping),
}
