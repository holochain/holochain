use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
};
use holochain_serialized_bytes::prelude::*;

/// Yaml supports key/value mappings where both the key and value can be any
/// yaml type, including fun things like NaN and Infinity as keys.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Mapping(HashMap<Value, Value>);

impl Mapping {
    /// Returns the value corresponding to the key in the map.
    #[inline]
    pub fn get(&self, k: &Value) -> Value {
        match self.0.get(k) {
            Some(v) => v.to_owned(),
            None => Value::Null,
        }
    }
}

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

/// Yaml supports floats, signed ints and even ints larger than `i64::MAX`.
/// To cover all these off we always attempt to put ints into a `u64` if they
/// are greater than or equal to zero, as `Number::PosInt` then fallback to
/// `Number::NegInt` if the integer is negative.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Number {
    /// All integers greater than or equal to zero.
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
                0.hash(state);
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

/// Yaml has types but allows every type in every position. For example, a
/// single key/value mapping could look like:
///
/// foo:
///     bar: 2
///     baz: bing
///     -3: 6.0
///
/// Which includes strings and integers in keys and strings, integers and float
/// values. It also doesn't enforce existence of anything in particular like a
/// struct/enum type or schema would. The integers can also exceed the ranges
/// supported by any individual rust integer type.
///
/// Of course happs are not forced to be so forgiving in the data they are
/// willing to accept and interpret in properites. But they MAY want to, so
/// the `Value` enum allows arbitrarily nested yaml data of any yaml type.
///
/// This is very similar to `serde_yaml::Value` but with more focussed scope,
/// notably not bringing serialization/deserialization logic for yaml data into
/// `holochain_zome_types`, where it would then be compiled into happ WASM. It
/// also has better support for keys that are floats than `serde_yaml`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Eq, PartialEq, Hash, SerializedBytes)]
pub enum Value {
    /// Analogous to `None` or `()`.
    Null,
    /// Same as Rust `bool`.
    Bool(bool),
    /// Either a `u64`, `i64` or `f64`.
    Number(Number),
    /// Same as Rust `String`.
    String(String),
    /// A vector of `Value` where each item may be any (different) type.
    Sequence(Vec<Value>),
    /// A `HashMap` where every key and value is any (different) `Value` type.
    Mapping(Mapping),
}