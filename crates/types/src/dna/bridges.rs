use crate::dna::fn_declarations::Trait;
use holochain_persistence_api::cas::content::Address;

use std::collections::BTreeMap;

/// A bridge is the definition of a connection to another DNA that runs under the same agency,
/// i.e. in the same conductor.
///
/// Defining a bridge means that the code in this DNA can call zome functions of that other
/// DNA.
///
/// The other DNA can either be referenced statically by exact DNA address/hash or dynamically
/// by defining the traits that other DNA has to provide in order to be usable as bridge.
///
/// Bridges can be required or optional. If a required bridge DNA is not installed this DNA
/// can't run, so required bridges are hard dependencies that have to be enforced by the conductor.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash)]
pub struct Bridge {
    /// Required or optional
    pub presence: BridgePresence,

    /// An arbitrary name of this bridge that is used as handle to reference this
    /// bridge in according zome API functions
    pub handle: String,

    /// Define what other DNA(s) to bridge to
    pub reference: BridgeReference,
}

/// This enum represents the two different ways of referring to another DNA instance.
/// If we know a priori what exact version of another DNA we want to bridge to we can
/// specify the DNA address (i.e. hash) and lock it in.
/// Often, we need more flexibility when
/// * the other DNA gets replaced by a newer version
/// * the other DNA gets created from a template and thus we don't know the exact hash
///   during build-time
/// * we want to build a complex system of components that should be pluggable.
/// Bridges can therefore also be specified by traits.
/// That means we specify a list of functions with their signatures and allow the conductor
/// (through the conductor bridge config) to resolve this bridge by any DNA instance that
/// implements all specified functions, just like a dynamic binding of function calls.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash)]
#[serde(untagged)]
pub enum BridgeReference {
    /// A bridge reference that defines another DNA statically by its address (i.e. hash).
    /// If this variant is used the other DNA gets locked in as per DNA address
    Address { dna_address: Address },

    /// A bridge reference that defines another DNA loosely by expecting a DNA that implements
    /// a given set of traits, i.e. that has specific sets of zome functions with
    /// matching signatures.
    Trait { traits: BTreeMap<String, Trait> },
}

/// Required or optional
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum BridgePresence {
    /// A required bridge is a dependency to another DNA.
    /// This DNA won't load without it.
    Required,

    /// An optional bridge may be missing.
    /// This DNA's code can check via API functions if the other DNA is installed and connected.
    Optional,
}
