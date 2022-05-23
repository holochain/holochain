/// Errors related to the secure primitive macro.
#[derive(Debug)]
pub enum CounterSigningError {
    /// Agent index is out of bounds for the signing session.
    AgentIndexOutOfBounds,
    /// An empty vector was used to build session data.
    MissingResponse,
    /// Session responses needs to be same length as the signing agents.
    CounterSigningSessionResponsesLength(usize, usize),
    /// Session response agents all need to be in the correct positions.
    CounterSigningSessionResponsesOrder(u8, usize),
    /// Enzyme must match for required and optional signers if set.
    EnzymeMismatch(holo_hash::AgentPubKey, holo_hash::AgentPubKey),
    /// If there are optional signers the session MUST be enzymatic.
    NonEnzymaticOptionalSigners,
    /// Agents length cannot be longer than max or less than min.
    AgentsLength(usize),
    /// Optional agents length cannot be shorter then minimum.
    OptionalAgentsLength(u8, usize),
    /// Optional agents length must be majority of the signers list.
    MinOptionalAgents(u8, usize),
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
            CounterSigningError::EnzymeMismatch(required_signer, optional_signer) => write!(f,
                "The enzyme is mismatche for required signer {} and optional signer {}",
                required_signer, optional_signer

            ),
            CounterSigningError::NonEnzymaticOptionalSigners => write!(f, "There are optional signers without an enzyme."),
            CounterSigningError::AgentsLength(len) => {
                write!(f, "The signing agents list is too long or short {}", len)
            },
            CounterSigningError::OptionalAgentsLength(min, len) => {
                write!(f, "The optional signing agents list length is {} which is less than the minimum {} required to sign", len, min)
            },
            CounterSigningError::MinOptionalAgents(min, len) => {
                write!(f, "The minimum optional agents {} is not a majority of {}", min, len)
            },
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
