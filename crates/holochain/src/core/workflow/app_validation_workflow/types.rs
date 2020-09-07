use holo_hash::AnyDhtHash;

#[derive(Debug)]
/// The outcome of sys validation
pub(super) enum Outcome {
    /// Moves to integration
    Accepted,
    /// Stays in limbo because a
    /// dependency needs is required to validate
    /// and could not be found
    AwaitingDeps(Vec<AnyDhtHash>),
    /// Moves to integration with status rejected
    Rejected(String),
}
