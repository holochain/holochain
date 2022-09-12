//! # Chain Activity
//! This module gives users the ability to use chain activity within validation
//! in a deterministic way.

use crate::prelude::*;
use holo_hash::AgentPubKey;

/// The chain this filter produces on the given agents chain
/// must be fetched before the validation can be completed.
/// This allows for deterministic validation of chain activity by
/// making a hash bounded range of an agents chain into a dependency
/// for something that is being validated.
///
/// Check the [`ChainFilter`] docs for more info.
pub fn must_get_agent_activity(
    author: AgentPubKey,
    filter: ChainFilter,
) -> ExternResult<Vec<RegisterAgentActivity>> {
    HDI.with(|h| {
        h.borrow()
            .must_get_agent_activity(MustGetAgentActivityInput {
                author,
                chain_filter: filter,
            })
    })
}
