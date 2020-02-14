use super::error::ChainInvalidReason;
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
        let reader = self.persistence.create_cursor()?;
        let head = Self::head_inner(&reader)?.ok_or(SourceChainError::ChainEmpty)?;
        Ok(SourceChainSnapshot { reader, head })
    }

    pub fn as_at(&self, head: ChainTop) -> SourceChainResult<SourceChainSnapshot> {
        Ok(SourceChainSnapshot {
            reader: self.persistence.create_cursor()?,
            head,
        })
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
        Ok(self.persistence.create_cursor()?)
    }

    /// TODO: rewrite once we have the multi-LMDB cursors sorted out, so that we can
    /// read the chain head from a different DB
    fn head_inner(reader: &Cursor) -> SourceChainResult<Option<ChainTop>> {
        let maybe_content = reader.fetch(&CHAIN_HEAD_ADDRESS)?;
        let maybe_address = match maybe_content {
            Some(content) => Some(HashString::try_from(content)?),
            None => None,
        };
        Ok(maybe_address.map(ChainTop))
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
        writer.add(&dna_entry)?;
        writer.add(&dna_header)?;
        writer.add(&head)?;

        let agent_entry = Entry::AgentId(agent.clone());
        let agent_header = self.header_for_entry(
            Some(&head),
            &agent_entry,
            &[Provenance::new(agent.address(), Signature::fake())],
            chrono::Utc::now().timestamp().into(),
        )?;
        let head = ChainTop(agent_header.address());

        writer.add(&agent_entry)?;
        writer.add(&agent_header)?;
        writer.add(&head)?;
        writer.commit()?;

        Ok(())
    }

    pub fn dna(&self) -> SourceChainResult<Dna> {
        let snapshot = self.now()?;
        let entry = snapshot.latest_entry_of_type(EntryType::Dna)?.ok_or(SourceChainError::InvalidStructure(ChainInvalidReason::GenesisMissing))?;
        if let Entry::Dna(dna) = entry {
            Ok(*dna)
        } else {
            Err(SourceChainError::InvalidStructure(
                ChainInvalidReason::HeaderAndEntryMismatch(entry.address()),
            ))
        }
    }

    pub fn agent_id(&self) -> SourceChainResult<AgentId> {
        let snapshot = self.now()?;
        let entry = snapshot.latest_entry_of_type(EntryType::AgentId)?.ok_or(SourceChainError::InvalidStructure(ChainInvalidReason::GenesisMissing))?;
        if let Entry::AgentId(agent) = entry {
            Ok(agent)
        } else {
            Err(SourceChainError::InvalidStructure(
                ChainInvalidReason::HeaderAndEntryMismatch(entry.address()),
            ))
        }
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

/// Temporary bastardization until we have LMDB transactions across even more DBs,
/// so that we can store the chain head in a different DB
/// TODO: remove once we've got that
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

    /// Check that the chain is structured properly:
    /// - Starts with Dna
    /// - Agent follows immediately after
    pub fn is_initialized(&self) -> SourceChainResult<bool> {
        use crate::agent::validity::ChainStructureInspectorState::{BothFound, NoneFound};

        let final_state = self
            .iter_back()
            .fold(NoneFound, |s, header| Ok(s.check(&header)))?;

        Ok(final_state == BothFound)
    }

    /// Perform a more rigorous check of the chain structure to see that it is valid
    /// TODO: check for missing CAS entries, etc., but for now just check for initialization
    pub fn validate(&self) -> SourceChainResult<()> {
        if self.is_initialized()? {
            Ok(())
        } else {
            Err(SourceChainError::InvalidStructure(
                ChainInvalidReason::GenesisMissing,
            ))
        }
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

    fn latest_entry_of_type(&self, entry_type: EntryType) -> SourceChainResult<Option<Entry>> {
        if let Some(header) = self
            .iter_back()
            .find(|h| Ok(*h.entry_type() == entry_type))?
        {
            let entry_address = header.entry_address();
            if let Some(content) = self.reader.fetch(entry_address)? {
                let entry = Entry::try_from(content)?;
                Ok(Some(entry))
            } else {
                Err(SourceChainError::InvalidStructure(
                    ChainInvalidReason::MissingData(entry_address.clone()),
                ))
            }
        } else {
            Ok(None)
        }
    }
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
                if let Some(content) = self.reader.fetch(head.address())? {
                    let header: ChainHeader = ChainHeader::try_from_content(&content)?;
                    self.current = header.link().map(ChainTop);
                    Ok(Some(header))
                } else {
                    Ok(None)
                }
            }
        }
    }
}

#[cfg(test)]
pub mod tests {

    use super::*;
    use crate::{
        cell::CellId, test_utils::fake_cell_id, txn::source_chain::SourceChainPersistence,
    };
    use std::collections::BTreeMap;
    use sx_types::test_utils::test_dna;
    use tempdir::TempDir;
    use Entry;

    fn test_initialized_chain(
        dna: Dna,
        agent: AgentId,
        persistence: &SourceChainPersistence,
    ) -> SourceChain {
        let dna: Dna = test_dna();
        let agent = AgentId::generate_fake("a");
        let id: CellId = (dna.address(), agent.clone());
        let chain = SourceChain::new(&persistence);
        let writer = persistence.create_cursor_rw().unwrap();
        chain.initialize(writer, dna, agent).unwrap();
        assert!(chain.validate().is_ok());
        chain
    }

    fn fake_header_for_entry(chain: &SourceChain, entry: &Entry, head: &ChainTop) -> ChainHeader {
        let provenances = &[Provenance::new(
            chain.agent_id().unwrap().address(),
            Signature::fake(),
        )];
        let timestamp = chrono::Utc::now().timestamp().into();

        ChainHeader::new(
            entry.entry_type(),
            entry.address(),
            provenances,
            Some(head.address().clone()),
            None,
            None,
            timestamp,
        )
    }

    #[test]
    fn detect_chain_initialized() {
        let dna: Dna = test_dna();
        let agent = AgentId::generate_fake("a");
        let id: CellId = (dna.address(), agent.clone());
        let tmpdir = TempDir::new("skunkworx").unwrap();
        let persistence = SourceChainPersistence::test(tmpdir.path());
        let chain = SourceChain::new(&persistence);
        let writer = persistence.create_cursor_rw().unwrap();

        assert_eq!(chain.validate(), Err(SourceChainError::ChainEmpty));

        chain.initialize(writer, dna, agent).unwrap();

        assert!(chain.validate().is_ok());
    }

    #[test]
    fn chain_writes_are_protected() {
        let tmpdir = TempDir::new("skunkworx").unwrap();
        let persistence = SourceChainPersistence::test(tmpdir.path());
        let chain = test_initialized_chain(test_dna(), AgentId::generate_fake("a"), &persistence);
        let post_init_head = chain.head().unwrap();

        let writer1 = persistence.create_cursor_rw().unwrap();
        let entry1 = Entry::App("type".into(), "content 1".into());
        let header1 = fake_header_for_entry(&chain, &entry1, &post_init_head);
        writer1.add(&entry1).unwrap();
        writer1.add(&header1).unwrap();

        let writer2 = persistence.create_cursor_rw().unwrap();
        let entry2 = Entry::App("type".into(), "content 2".into());
        let header2 = fake_header_for_entry(&chain, &entry2, &post_init_head);
        writer2.add(&entry1).unwrap();
        writer2.add(&header1).unwrap();

        assert_eq!(chain.try_commit(writer1), Ok(()));
        // TODO: replace this assertion with the actual error issuing from multiple writes, once we know what it is
        assert_ne!(chain.try_commit(writer2), Ok(()));
    }
}
