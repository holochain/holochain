use crate::UnweighedCountersigningHeader;

/// Errors related to the secure primitive macro.
#[derive(Debug)]
pub enum CounterSigningError {
    /// Agent index is out of bounds for the signing session.
    AgentIndexOutOfBounds,
    /// An empty vector was used to build session data.
    MissingResponse,
    /// The header does not correspond to an app entry type.
    HeaderNotAppEntry(Box<UnweighedCountersigningHeader>),
    /// Session responses needs to be same length as the signing agents.
    CounterSigningSessionResponsesLength(usize, usize),
    /// Session response agents all need to be in the correct positions.
    CounterSigningSessionResponsesOrder(u8, usize),
    /// Enzyme index must be one of the signers if set.
    EnzymeIndex(usize, usize),
    /// Agents length cannot be longer than max or less than min.
    AgentsLength(usize),
    /// There cannot be duplicates in the agents list.
    AgentsDupes(Vec<holo_hash::AgentPubKey>),
    /// The session times must validate.
    CounterSigningSessionTimes(crate::CounterSigningSessionTimes),
}

impl std::error::Error for CounterSigningError {}

impl core::fmt::Display for CounterSigningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CounterSigningError::AgentIndexOutOfBounds => {
                write!(f, "Agent index is out of bounds for the signing session.")
            }
            CounterSigningError::MissingResponse => write!(
                f,
                "Attempted to build CounterSigningSessionData with an empty response vector."
            ),
            CounterSigningError::HeaderNotAppEntry(h) => write!(
                f,
                "The countersigning header does not correspond to an app entry type: {:?}.",
                h
            ),
            CounterSigningError::CounterSigningSessionResponsesLength(resp, num_agents) => {
                write!(f,
                    "The countersigning session responses ({}) did not match the number of signing agents ({})",
                    resp,
                    num_agents
                )
            }
            CounterSigningError::CounterSigningSessionResponsesOrder(index, pos) => write!(f,
                    "The countersigning session response with agent index {} was found in index position {}",
                    index, pos
            ),
            CounterSigningError::EnzymeIndex(len, index) => write!(f,
                "The enzyme index {} is out of bounds for signing agents list of length {}",
                index, len

            ),
            CounterSigningError::AgentsLength(len) => {
                write!(f, "The signing agents list is too long or short {}", len)
            }
            CounterSigningError::AgentsDupes(agents) => write!(
                f,
                "The signing agents list contains duplicates {:?}",
                agents
            ),
            CounterSigningError::CounterSigningSessionTimes(times) => write!(
                f,
                "The countersigning session times were not valid {:?}",
                times
            ),
        }
    }
}
