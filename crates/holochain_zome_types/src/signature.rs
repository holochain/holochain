//! Signature for authenticity of data

use crate::prelude::Bytes;
use holo_hash::AgentPubKey;
use holochain_integrity_types::signature::Signature;
use holochain_serialized_bytes::prelude::*;

/// Input structure for creating a signature.
#[derive(Debug, PartialEq, Serialize, Deserialize, SerializedBytes, Clone)]
pub struct Sign {
    /// The public key associated with the private key that should be used to
    /// generate the signature.
    pub key: holo_hash::AgentPubKey,

    /// The data that should be signed.
    pub data: Bytes,
}

impl Sign {
    /// construct a new Sign struct.
    pub fn new<S>(key: holo_hash::AgentPubKey, input: S) -> Result<Self, SerializedBytesError>
    where
        S: Serialize + std::fmt::Debug,
    {
        Ok(Self::new_raw(
            key,
            holochain_serialized_bytes::encode(&input)?,
        ))
    }

    /// construct a new Sign struct from raw bytes.
    pub fn new_raw(key: holo_hash::AgentPubKey, data: Vec<u8>) -> Self {
        Self {
            key,
            data: Bytes::from(data),
        }
    }

    /// key getter
    pub fn key(&self) -> &AgentPubKey {
        &self.key
    }

    /// data getter
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

/// Ephemerally sign a vector of bytes (i.e. a `Vec<Vec<u8>>`)
/// Each of the items of the outer vector represents something to sign
/// and will have a corresponding Signature in the output.
/// The public key for the ephemeral operation will be returned in the output.
/// Structurally mirrors/complements the `Signature` struct as a new type.
/// There we know the key on the input side, here we receive the key on the output.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(transparent)]
pub struct SignEphemeral(pub Vec<Bytes>);

impl SignEphemeral {
    /// Construct a new SignEphemeral from a vector of Serialize inputs.
    /// The signing key will be generated and discarded by the host.
    pub fn new<S>(inputs: Vec<S>) -> Result<Self, SerializedBytesError>
    where
        S: Serialize + std::fmt::Debug,
    {
        let datas: Result<Vec<_>, _> = inputs
            .into_iter()
            .map(|s| holochain_serialized_bytes::encode(&s))
            .collect();
        Ok(Self::new_raw(datas?))
    }

    /// Construct a SignEphemeral from a vector of bytes.
    pub fn new_raw(datas: Vec<Vec<u8>>) -> Self {
        Self(datas.into_iter().map(Bytes::from).collect())
    }

    /// Consumes self.
    pub fn into_inner(self) -> Vec<Bytes> {
        self.0
    }
}

/// Some data with a signature attached.
///
/// Note that this is not a desirable pattern, because we sign serialized data,
/// and associating the signature with the unserialized data means that if the
/// serialization changes at all, the signature will no longer be valid.
/// We should structure our flows to only handle signatures in the context of
/// serialized data, and this kind of type should reflect that.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    derive_more::Constructor,
    derive_more::Deref,
    derive_more::From,
    derive_more::Into,
)]
pub struct Signed<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    #[deref]
    #[serde(bound(deserialize = "T: serde::de::DeserializeOwned"))]
    data: T,
    signature: Signature,
}

impl<T> Signed<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    /// Accessor for the signed data
    pub fn data(&self) -> &T {
        &self.data
    }

    /// Accessor for the signed data
    pub fn into_data(self) -> T {
        self.data
    }

    /// Accessor for the Signature
    pub fn signature(&self) -> &Signature {
        &self.signature
    }
}

// Same failure mode as `HoloHashed<C>` (`holo_hash::hashed`) and
// `SignedHashed<T>` (`holochain_integrity_types::record`): `WithoutGenerics`
// substitutes `Dummy` for `T`, which doesn't implement
// `Serialize`/`DeserializeOwned`. Hand-written instead, declared as a
// genuine generic type so ordinary fields naming `Signed<T>` resolve their
// `Signature` import correctly.
#[cfg(feature = "ts_rs")]
impl<T> ts_rs::TS for Signed<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + ts_rs::TS,
{
    type WithoutGenerics = SignedWithoutGenerics;
    type OptionInnerType = Self;

    fn name(cfg: &ts_rs::Config) -> String {
        format!("Signed<{}>", T::name(cfg))
    }

    fn inline(cfg: &ts_rs::Config) -> String {
        format!(
            "{{ data: {}, signature: {} }}",
            T::name(cfg),
            <Signature as ts_rs::TS>::name(cfg)
        )
    }

    fn decl(_: &ts_rs::Config) -> String {
        "type Signed<T> = { data: T, signature: Signature };".into()
    }

    fn decl_concrete(cfg: &ts_rs::Config) -> String {
        format!("type Signed = {};", <Self as ts_rs::TS>::inline(cfg))
    }

    fn visit_dependencies(v: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        v.visit::<T>();
        T::visit_dependencies(v);
        v.visit::<Signature>();
    }

    fn visit_generics(v: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        T::visit_generics(v);
        v.visit::<T>();
    }

    fn output_path() -> Option<std::path::PathBuf> {
        Some("hdk/action.ts".into())
    }
}

/// [`Signed`]'s [`ts_rs::TS::WithoutGenerics`] — just the imports its fixed
/// declaration text references (`Signature`), unrelated to any concrete `T`.
#[cfg(feature = "ts_rs")]
#[doc(hidden)]
pub struct SignedWithoutGenerics;

#[cfg(feature = "ts_rs")]
impl ts_rs::TS for SignedWithoutGenerics {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name(_: &ts_rs::Config) -> String {
        "Signed".into()
    }

    fn inline(_: &ts_rs::Config) -> String {
        panic!("SignedWithoutGenerics is a type-level placeholder and cannot be inlined")
    }

    fn visit_dependencies(v: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        v.visit::<Signature>();
    }

    fn output_path() -> Option<std::path::PathBuf> {
        Some("hdk/action.ts".into())
    }
}
