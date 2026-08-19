//! Hand-written [`ts_rs::TS`] impls for `HoloHash` and its concrete aliases.
//!
//! `HoloHash<T>` serializes via a manual `serde` implementation (the raw 39
//! hash bytes, see `crate::ser`), so `ts-rs` cannot derive a `TS` impl for it.
//! Instead, every hash type is modeled on the TypeScript side as a
//! `Uint8Array`, aliased under its Rust name so the generated bindings stay
//! self-documenting.

use crate::*;
use ts_rs::TS;

/// Marker type emitting the base `HoloHash` TypeScript declaration.
///
/// Every concrete hash alias (`AgentPubKey`, `DnaHash`, ...) depends on this
/// declaration and is itself declared as `type X = HoloHash;`.
pub struct HoloHashTs;

impl TS for HoloHashTs {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name(_: &ts_rs::Config) -> String {
        "HoloHash".into()
    }

    fn inline(cfg: &ts_rs::Config) -> String {
        Self::name(cfg)
    }

    fn decl(_: &ts_rs::Config) -> String {
        "type HoloHash = Uint8Array;".into()
    }

    fn decl_concrete(cfg: &ts_rs::Config) -> String {
        Self::decl(cfg)
    }

    fn output_path() -> Option<std::path::PathBuf> {
        Some("types.ts".into())
    }
}

/// Implements [`TS`] for `HoloHash<$hash_type>`, declaring it as an alias of
/// [`HoloHashTs`] under the given TypeScript name.
macro_rules! holo_hash_ts {
    ($hash_type:ty, $name:literal) => {
        impl TS for HoloHash<$hash_type> {
            type WithoutGenerics = Self;
            type OptionInnerType = Self;

            fn name(_: &ts_rs::Config) -> String {
                $name.into()
            }

            fn inline(cfg: &ts_rs::Config) -> String {
                Self::name(cfg)
            }

            fn decl(_: &ts_rs::Config) -> String {
                concat!("type ", $name, " = HoloHash;").into()
            }

            fn decl_concrete(cfg: &ts_rs::Config) -> String {
                Self::decl(cfg)
            }

            fn visit_dependencies(v: &mut impl ts_rs::TypeVisitor)
            where
                Self: 'static,
            {
                v.visit::<HoloHashTs>();
            }

            fn output_path() -> Option<std::path::PathBuf> {
                Some("types.ts".into())
            }
        }
    };
}

holo_hash_ts!(hash_type::Agent, "AgentPubKey");
holo_hash_ts!(hash_type::Dna, "DnaHash");
holo_hash_ts!(hash_type::Wasm, "WasmHash");
holo_hash_ts!(hash_type::Entry, "EntryHash");
holo_hash_ts!(hash_type::Action, "ActionHash");
holo_hash_ts!(hash_type::External, "ExternalHash");
holo_hash_ts!(hash_type::DhtOp, "DhtOpHash");
holo_hash_ts!(hash_type::Warrant, "WarrantHash");
holo_hash_ts!(hash_type::AnyDht, "AnyDhtHash");
holo_hash_ts!(hash_type::AnyLinkable, "AnyLinkableHash");
holo_hash_ts!(hash_type::Inline, "InlineHash");

/// Declares a Rust type alias as a TypeScript type alias.
///
/// Generates a hidden unit marker type named `$marker` and a [`TS`] impl for
/// it that emits `type $name = $rhs;` into `$file`. List any TypeScript
/// types the alias refers to under `deps:` so `ts-rs` emits imports for them.
///
/// # Example
///
/// ```ignore
/// ts_alias!(TimestampTs, "Timestamp", "number", "types.ts", deps: []);
/// ```
///
/// Reference the marker from a field with `#[ts(as = "TimestampTs")]`, not
/// `#[ts(type = "Timestamp")]` — the latter splices in a bare string with no
/// backing type, so ts-rs registers no dependency and a cross-file reference
/// silently breaks.
#[macro_export]
macro_rules! ts_alias {
    ($marker:ident, $name:literal, $rhs:literal, $file:literal, deps: [$($dep:ty),* $(,)?]) => {
        #[doc(hidden)]
        pub struct $marker;

        impl ts_rs::TS for $marker {
            type WithoutGenerics = Self;
            type OptionInnerType = Self;

            fn name(_: &ts_rs::Config) -> String {
                $name.into()
            }

            fn inline(cfg: &ts_rs::Config) -> String {
                <Self as ts_rs::TS>::name(cfg)
            }

            fn decl(_: &ts_rs::Config) -> String {
                concat!("type ", $name, " = ", $rhs, ";").into()
            }

            fn decl_concrete(cfg: &ts_rs::Config) -> String {
                <Self as ts_rs::TS>::decl(cfg)
            }

            #[allow(
                unused_variables,
                reason = "`v` goes unused when a `ts_alias!` invocation lists no deps; \
                          cross-crate callers are exempt from this lint automatically, \
                          but same-crate callers (e.g. within holo_hash itself) are not"
            )]
            fn visit_dependencies(v: &mut impl ts_rs::TypeVisitor)
            where
                Self: 'static,
            {
                $(v.visit::<$dep>();)*
            }

            fn output_path() -> Option<std::path::PathBuf> {
                Some($file.into())
            }
        }
    };
}

/// Exports every TypeScript declaration this crate contributes to the
/// conductor API bindings: the hand-written hash impls in this module and
/// the `*B64` string aliases in `crate::hash_b64`. Downstream crates call
/// this from their own `export_ts_bindings` so the whole tree is written by
/// a single process — ts-rs only merges declarations sharing an output file
/// within one process.
pub fn export_ts_bindings(cfg: &ts_rs::Config) -> Result<(), ts_rs::ExportError> {
    HoloHashTs::export_all(cfg)?;
    crate::AgentPubKey::export_all(cfg)?;
    crate::DnaHash::export_all(cfg)?;
    crate::WasmHash::export_all(cfg)?;
    crate::EntryHash::export_all(cfg)?;
    crate::ActionHash::export_all(cfg)?;
    crate::ExternalHash::export_all(cfg)?;
    crate::DhtOpHash::export_all(cfg)?;
    crate::WarrantHash::export_all(cfg)?;
    crate::AnyDhtHash::export_all(cfg)?;
    crate::AnyLinkableHash::export_all(cfg)?;
    crate::InlineHash::export_all(cfg)?;
    #[cfg(feature = "encoding")]
    {
        crate::DnaHashB64Ts::export_all(cfg)?;
        crate::WasmHashB64Ts::export_all(cfg)?;
    }
    Ok(())
}

#[cfg(all(test, feature = "ts_rs"))]
mod ts_export {
    use super::*;

    #[test]
    fn hash_decls_have_expected_shape() {
        let cfg = ts_rs::Config::default();
        assert_eq!(HoloHashTs::decl(&cfg), "type HoloHash = Uint8Array;");
        assert_eq!(
            crate::AgentPubKey::decl(&cfg),
            "type AgentPubKey = HoloHash;"
        );
    }
}
