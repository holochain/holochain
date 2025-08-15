//! Helper types and [`ts_rs::TS`] implementations for converting hash types
//! into a form that can be exported to TypeScript for the JavaScript client.
//!
//! In the JavaScript client, all specific hash types are aliases to a 'base'
//! HoloHash type, which is just a Uint8Array. Likewise, all the specific
//! base64-encoded hash types are aliases to a HoloHashB64 type which is just
//! a string. This is something that ts-rs can't infer from the corresponding
//! Rust types and their serde implementations, because those implementations
//! are custom rather than simple proc macros.

use std::path::PathBuf;
use ts_rs::TS;
use crate::{HashType, HoloHash, HoloHashResult};
#[cfg(feature = "encoding")]
use crate::HoloHashB64;

// RAW HASH TYPES

/// The base hash type in TS definitions is `HoloHash`; all concrete types
/// are just aliases of it.
pub(crate) const BASE_HASH_NAME: &str = "HoloHash";

/// A hash type used for exporting the base `HoloHash` to TypeScript.
/// This is mostly here to satisfy ts-rs' requirement that a type's generic
/// parameter(s) be filled in with _something_ for its `WithoutGenerics` prop.
#[derive(Copy, Clone, std::fmt::Debug, std::hash::Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct BaseHashType;

impl HashType for BaseHashType {
    fn get_prefix(self) -> &'static [u8] { unimplemented!("This hash type should not be used outside of exporting to TypeScript") }

    fn try_from_prefix(_: &[u8]) -> HoloHashResult<Self> { unimplemented!("This hash type should not be used outside of exporting to TypeScript") }

    fn hash_name(self) -> &'static str { unimplemented!("This hash type should not be used outside of exporting to TypeScript") }

    fn static_hash_name() -> &'static str { BASE_HASH_NAME }

    fn is_base() -> bool { true }
}

impl TS for BaseHashType {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name() -> String { "".into() }

    fn inline() -> String { "".into() }

    fn inline_flattened() -> String {
        Self::inline()
    }

    fn decl() -> String { "".into() }

    fn decl_concrete() -> String {
        Self::decl()
    }
}

impl<T: HashType> TS for HoloHash<T> {
    type WithoutGenerics = HoloHash<BaseHashType>;
    type OptionInnerType = Self;

    fn name() -> String {
        T::static_hash_name().into()
    }

    fn inline() -> String {
        // The base HoloHash is a Uint8Array, whereas all specific hash types
        // are aliases to HoloHash.
        match T::is_base() {
            true => "Uint8Array".into(),
            false => BASE_HASH_NAME.into(),
        }
    }

    fn inline_flattened() -> String {
        Self::inline()
    }

    fn decl() -> String {
        format!("type {} = {};", Self::name(), Self::inline()).into()
    }

    fn decl_concrete() -> String {
        Self::decl()
    }

    // Not used -- only used to make ts-rs be quiet; otherwise it fails with
    // an "Error: this type cannot be exported". I suspect this is an upstream
    // bug but am not interested in fixing it.
    fn output_path() -> Option<PathBuf> {
        let mut path = PathBuf::new();
        path.push(".");
        Some(path)
    }

    // Ditto.
    fn default_output_path() -> Option<PathBuf> {
        let mut path = PathBuf::new();
        path.push(".");
        Some(path)
    }
}

/// The base type for all hashes.
pub type BaseHoloHash = HoloHash<BaseHashType>;

// BASE64-ENCODED HASH TYPES

#[cfg(feature = "encoding")]
impl<T: HashType> TS for HoloHashB64<T> {
    type WithoutGenerics = HoloHashB64<BaseHashType>;
    type OptionInnerType = Self;

    fn name() -> String {
        format!("{}B64", T::static_hash_name()).into()
    }

    fn inline() -> String {
        match T::is_base() {
            true => "string".into(),
            false => format!("{}B64", BASE_HASH_NAME).into(),
        }
    }

    fn inline_flattened() -> String {
        Self::inline()
    }

    fn decl() -> String {
        format!("type {} = {};", Self::name(), Self::inline()).into()
    }

    fn decl_concrete() -> String {
        Self::decl()
    }

    fn output_path() -> Option<PathBuf> {
        let mut path = PathBuf::new();
        path.push(".");
        Some(path)
    }

    fn default_output_path() -> Option<PathBuf> {
        let mut path = PathBuf::new();
        path.push(".");
        Some(path)
    }
}

/// The base type for all Base64-encoded hashes.
#[cfg(feature = "encoding")]
pub type BaseHoloHashB64 = HoloHashB64<BaseHashType>;
