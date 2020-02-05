use crate::{
    agent::error::{SourceChainError, SourceChainResult},
    cell::Cell,
    txn::{
        source_chain,
        source_chain::{Cursor, CursorRw},
    },
};
use fallible_iterator::FallibleIterator;
use holochain_json_api::error::JsonError;
use holochain_persistence_api::{
    cas::content::Address,
    txn::{CursorProvider, Writer},
};
use lazy_static::*;
use sx_types::{
    agent::AgentId,
    chain_header::ChainHeader,
    dna::Dna,
    entry::{entry_type::EntryType, Entry},
    error::{SkunkError, SkunkResult},
    prelude::*,
    shims::*,
    signature::{Provenance, Signature},
    time::Iso8601,
};

pub struct SourceChain<'a> {
    persistence: &'a source_chain::SourceChainPersistence,
}

impl<'a> SourceChain<'a> {
    pub(crate) fn new(persistence: &'a source_chain::SourceChainPersistence) -> Self {
        Self { persistence }
    }

    pub fn now(&self) -> SourceChainResult<SourceChainSnapshot> {
        let reader = self.persistence.create_cursor().map_err(SkunkError::from)?;
        let head = Self::head_inner(&reader)?.ok_or(SourceChainError::ChainEmpty)?;
        Ok(SourceChainSnapshot { reader, head })
    }

    pub fn validate(&self) -> SourceChainResult<()> {
        let _ = self.now()?;
        Ok(())
    }

    fn header_for_entry(
        &self,
        chain_head: Option<&ChainTop>,
        entry: &Entry,
        provenances: &[Provenance],
        timestamp: Iso8601,
    ) -> SourceChainResult<ChainHeader> {
        let link = chain_head.map(|head| head.address().clone());
        if link.is_none() && entry.entry_type() != EntryType::Dna {
            error!("Attempting to create header for non-Dna entry, but the chain is empty");
            return Err(SourceChainError::ChainEmpty);
        }
        let header = ChainHeader::new(
            entry.entry_type(),
            entry.address(),
            provenances,
            link,
            None, // TODO
            None, // TODO!!
            timestamp,
        );
        Ok(header)
    }

    /// Return the current chain top address. If no top is persisted, this is treated an error.
    pub fn head(&self) -> SourceChainResult<ChainTop> {
        let reader = self.reader()?;
        Self::head_inner(&reader)?.ok_or(SourceChainError::ChainEmpty)
    }

    fn reader(&self) -> SourceChainResult<Cursor> {
        Ok(self.persistence.create_cursor().map_err(SkunkError::from)?)
    }

    fn head_inner(reader: &Cursor) -> SourceChainResult<Option<ChainTop>> {
        let maybe_content = reader
            .fetch(&CHAIN_HEAD_ADDRESS)
            .map_err(SkunkError::from)?;
        let maybe_address = match maybe_content {
            Some(content) => Some(HashString::try_from(content).map_err(SkunkError::from)?),
            None => None,
        };
        Ok(maybe_address.map(ChainTop))
    }

    pub fn as_at(&self, head: ChainTop) -> SourceChainResult<SourceChainSnapshot> {
        Ok(SourceChainSnapshot {
            reader: self.persistence.create_cursor().map_err(SkunkError::from)?,
            head,
        })
    }

    pub fn initialize(&self, writer: CursorRw, dna: Dna, agent: AgentId) -> SourceChainResult<()> {
        let dna_entry = Entry::Dna(Box::new(dna));
        let dna_header = self.header_for_entry(
            None,
            &dna_entry,
            &[Provenance::new(agent.address(), Signature::fake())],
            chrono::Utc::now().timestamp().into(),
        )?;
        let head = ChainTop(dna_header.address());
        writer.add(&dna_entry).map_err(SkunkError::from)?;
        writer.add(&dna_header).map_err(SkunkError::from)?;
        writer.add(&head).map_err(SkunkError::from)?;

        let agent_entry = Entry::AgentId(agent.clone());
        let agent_header = self.header_for_entry(
            Some(&head),
            &agent_entry,
            &[Provenance::new(agent.address(), Signature::fake())],
            chrono::Utc::now().timestamp().into(),
        )?;
        let head = ChainTop(agent_header.address());

        writer.add(&agent_entry).map_err(SkunkError::from)?;
        writer.add(&agent_header).map_err(SkunkError::from)?;
        writer.add(&head).map_err(SkunkError::from)?;
        writer.commit().map_err(SkunkError::from)?;

        Ok(())
    }

    pub fn dna(&self) -> SkunkResult<Dna> {
        unimplemented!()
    }

    pub fn agent_id(&self) -> SkunkResult<AgentId> {
        unimplemented!()
    }

    /// Use the SCHH to attempt to write a bundle of changes
    pub fn try_commit(&self, cursor_rw: source_chain::CursorRw) -> SkunkResult<()> {
        Ok(cursor_rw.commit()?)
    }
}

lazy_static! {
    static ref CHAIN_HEAD_ADDRESS: HashString = HashString::from("chain-head");
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChainTop(Address);

impl ChainTop {
    pub fn address(&self) -> &Address {
        &self.0
    }
}

// impl From<Address> for ChainTop {
//     fn from(x: Address) -> ChainTop {
//         ChainTop(x)
//     }
// }

impl AddressableContent for ChainTop {
    fn address(&self) -> Address {
        CHAIN_HEAD_ADDRESS.clone()
    }

    fn content(&self) -> Content {
        self.0.clone().into()
    }

    fn try_from_content(content: &Content) -> Result<Self, JsonError> {
        Ok(Self(HashString::try_from(content.clone())?))
    }
}

/// Representation of a Cell's source chain.
/// TODO: work out the details of what's needed for as-at
/// to make sure the right balance is struck between
/// creating as-at snapshots and having access to the actual current source chain
pub struct SourceChainSnapshot {
    reader: source_chain::Cursor,
    head: ChainTop,
}

impl SourceChainSnapshot {
    /// Fails if a source chain has not yet been created for this CellId.
    fn new(reader: source_chain::Cursor, head: ChainTop) -> SourceChainResult<Self> {
        match reader.contains(head.address()) {
            Ok(true) => Ok(Self { reader, head }),
            Ok(false) => Err(SourceChainError::MissingHead),
            Err(e) => Err(SkunkError::from(e).into()),
        }
    }

    pub fn is_initialized(&self) -> SourceChainResult<bool> {
        use ChainInitDetectionState::*;

        let final_state = self.iter_back().fold(NoneFound, |s, header| {
            Ok(match header.entry_type() {
                EntryType::Dna => s.found_dna(),
                EntryType::AgentId => s.found_agent(),
                _ => s,
            })
        })?;

        Ok(final_state == BothFound)
    }

    /// Check that the chain is structured properly:
    /// - Starts with Dna
    /// - Agent follows immediately after
    pub fn validate(&self) -> SourceChainResult<()> {
        // TODO
        unimplemented!()
    }

    pub fn iter_back(&self) -> SourceChainBackwardIterator {
        SourceChainBackwardIterator {
            reader: self.reader.clone(),
            current: Some(self.head.clone()),
        }
    }

    // TODO
    // pub fn iter_forth(&self) -> SourceChainForwardIterator {
    //     unimplemented!()
    // }
}

pub struct SourceChainBackwardIterator {
    reader: source_chain::Cursor,
    current: Option<ChainTop>,
}

/// Follows ChainHeader.link through every previous Entry (of any EntryType) in the chain
// #[holochain_tracing_macros::newrelic_autotrace(HOLOCHAIN_CORE)]
impl FallibleIterator for SourceChainBackwardIterator {
    type Item = ChainHeader;
    type Error = SourceChainError;

    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        match &self.current {
            None => Ok(None),
            Some(head) => {
                if let Some(content) = self
                    .reader
                    .fetch(head.address())
                    .map_err(SkunkError::from)?
                {
                    let header: ChainHeader =
                        ChainHeader::try_from_content(&content).map_err(SkunkError::from)?;
                    self.current = header.link().map(ChainTop);
                    Ok(Some(header))
                } else {
                    Ok(None)
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ChainInitDetectionState {
    NoneFound,
    AgentFound,
    DnaFound,
    BothFound,
}

use ChainInitDetectionState::*;

impl ChainInitDetectionState {
    fn found_dna(self) -> Self {
        match self {
            NoneFound => DnaFound,
            AgentFound => BothFound,
            s => s,
        }
    }

    fn found_agent(self) -> Self {
        match self {
            NoneFound => AgentFound,
            DnaFound => BothFound,
            s => s,
        }
    }
}

#[cfg(test)]
pub mod tests {

    use super::*;
    use crate::{
        cell::CellId, test_utils::fake_cell_id, txn::source_chain::SourceChainPersistence,
    };
    use sx_types::test_utils::test_dna;

    #[test]
    fn chain_init_detection_state() {
        use ChainInitDetectionState::*;

        assert_eq!(NoneFound.found_agent(), AgentFound);
        assert_eq!(NoneFound.found_dna(), DnaFound);

        assert_eq!(AgentFound.found_agent(), AgentFound);
        assert_eq!(DnaFound.found_dna(), DnaFound);

        assert_eq!(AgentFound.found_dna(), BothFound);
        assert_eq!(DnaFound.found_agent(), BothFound);

        assert_eq!(BothFound.found_agent(), BothFound);
        assert_eq!(BothFound.found_dna(), BothFound);
    }

    #[test]
    fn detect_chain_initialized() {
        let dna: Dna = test_dna();
        let agent = AgentId::generate_fake("a");
        let id: CellId = (dna.address(), agent.clone());
        let persistence = SourceChainPersistence::test(id);
        let chain = SourceChain::new(&persistence);
        let writer = persistence.create_cursor_rw().unwrap();

        assert_eq!(chain.validate(), Err(SourceChainError::ChainEmpty));

        chain.initialize(writer, dna, agent).unwrap();

        assert!(chain.validate().is_ok());
    }
}
