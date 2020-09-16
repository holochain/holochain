//! the _host_ types used to track the status/result of validating entries
//! c.f. _guest_ types for validation callbacks and packages across the wasm boudary in zome_types

/// the validation status for an op
/// much of this happens in the subconscious
/// an entry missing validation dependencies may cycle through Pending many times before finally
/// reaching a final validation state or being abandoned
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ValidationStatus {
    /// all implemented validation callbacks found all dependencies and passed validation
    Valid,
    /// some implemented validation callback definitively failed validation
    Rejected,
    /// the subconscious has decided to never again attempt a conscious validation
    /// commonly due to missing validation dependencies remaining missing for "too long"
    Abandoned,
}
