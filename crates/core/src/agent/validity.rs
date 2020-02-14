/// Tiny state machine to handle the logic of discovering
/// whether a chain is valid when traversing it backwards.
/// Just `fold` over the chain with `NoneFound`
/// as the initial accumulator and do a `check` on each header.
///
/// There is one intermediate state, AgentFound, which is
/// when we've found the AgentId but not the DNA entry
/// (which should be next)
///
/// In the future we can add more validity checks. For now,
/// all this is doing is simply checking that the first two headers
/// have entry type Dna and AgentId, respectively.

use sx_types::{entry::entry_type::EntryType, chain_header::ChainHeader};

#[derive(Debug, PartialEq, Eq)]
pub enum ChainStructureInspectorState {
    NoneFound,
    AgentFound,
    BothFound,
    Invalid,
}

use ChainStructureInspectorState::*;

impl ChainStructureInspectorState {

    pub fn check(self, header: &ChainHeader) -> Self {
        match header.entry_type() {
            EntryType::Dna => self.found_dna(),
            EntryType::AgentId => self.found_agent(),
            _ => self.found_other(),
        }
    }

    fn found_agent(self) -> Self {
        match self {
            NoneFound => AgentFound,
            _ => Invalid
        }
    }

    fn found_dna(self) -> Self {
        match self {
            AgentFound => BothFound,
            _ => Invalid,
        }
    }

    fn found_other(self) -> Self {
        match self {
            NoneFound => NoneFound,
            _ => Invalid,
        }
    }
}


#[cfg(test)]
pub mod tests {

    use super::*;

    #[test]
    fn chain_init_detection_state() {
        use ChainStructureInspectorState::*;

        assert_eq!(NoneFound.found_agent(), AgentFound);
        assert_eq!(NoneFound.found_dna(), Invalid);
        assert_eq!(NoneFound.found_other(), NoneFound);

        assert_eq!(AgentFound.found_agent(), Invalid);
        assert_eq!(AgentFound.found_dna(), BothFound);
        assert_eq!(AgentFound.found_other(), Invalid);

        assert_eq!(BothFound.found_agent(), Invalid);
        assert_eq!(BothFound.found_dna(), Invalid);
        assert_eq!(BothFound.found_other(), Invalid);
    }
}