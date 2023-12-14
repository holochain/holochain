use super::*;

#[derive(Debug, PartialEq, Eq)]
/// The outcome of sys validation
pub(crate) enum Outcome {
    /// Moves to app validation
    Accepted,
    /// Stays in limbo because a dependency could not
    /// be found currently on the DHT.
    /// Note this is not proof it doesn't exist.
    MissingDhtDep(AnyDhtHash),
    /// Moves to integration with status rejected, with an informational reason
    Rejected(String),
}
