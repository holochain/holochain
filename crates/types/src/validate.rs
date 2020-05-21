//! the _host_ types used to track the status/result of validating entries
//! c.f. _guest_ types for validation callbacks and packages across the wasm boudary in zome_types

/// the validation status for an entry
/// much of this happens in the subconscious
/// an entry missing validation dependencies may cycle through Pending many times before finally
/// reaching a final validation state or being abandoned
pub enum ValidationStatus {
    /// all implemented validation callbacks found all dependencies and passed validation
    Valid,
    /// some implemented validation callback definitively failed validation
    Invalid,
    /// the subconscious is not yet ready to attempt a conscious validation
    /// commonly due to retry/backoff process due to missing validation dependencies
    Pending,
    /// the subconscious has decided to never again attempt a conscious validation
    /// commonly due to missing validation dependencies remaining missing for "too long"
    Abandoned,
}
