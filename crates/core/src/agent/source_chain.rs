use super::{error::ChainInvalidReason, SourceChainCommitBundle, SourceChainSnapshot};
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

/// Interface to the source chain as accessed through persistent storage.
/// From a SourceChain, you can construct a SourceChainSnapshot to make queries,
/// or a SourceChainCommitBundle, to start a write transaction and potentially commit
/// the changes later with `try_commit`.
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
        SourceChainSnapshot::new(reader, head)
    }

    pub fn as_at(&self, head: ChainTop) -> SourceChainResult<SourceChainSnapshot> {
        SourceChainSnapshot::new(self.persistence.create_cursor()?, head)
    }

    pub fn bundle(&self) -> SourceChainResult<SourceChainCommitBundle> {
        let cursor = self.persistence.create_cursor_rw()?;
        let head = Self::head_inner(&cursor)?.ok_or(SourceChainError::ChainEmpty)?;
        SourceChainCommitBundle::new(cursor, head)
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
        self.now()?.dna()
    }

    pub fn agent_id(&self) -> SourceChainResult<AgentId> {
        self.now()?.agent_id()
    }

    /// Use the SCHH to attempt to write a bundle of changes
    pub fn try_commit(&self, bundle: SourceChainCommitBundle) -> SourceChainResult<()> {
        let bundle_head = bundle.original_head();
        let self_head = self.head()?;
        if *bundle_head == self_head {
            Ok(bundle.commit()?)
        } else {
            Err(SourceChainError::HeadMismatch(
                bundle_head.clone(),
                self_head,
            ))
        }
    }
}

lazy_static! {
    static ref CHAIN_HEAD_ADDRESS: HashString = HashString::from("chain-head");
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChainTop(Address);

impl ChainTop {
    pub fn new(address: Address) -> Self {
        Self(address)
    }

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
    fn chains_are_protected_from_concurrent_transactional_writes_aka_as_at() {
        let tmpdir = TempDir::new("skunkworx").unwrap();
        let persistence = SourceChainPersistence::test(tmpdir.path());
        let chain = test_initialized_chain(test_dna(), AgentId::generate_fake("a"), &persistence);
        let post_init_head = chain.head().unwrap();

        let mut bundle1 = chain.bundle().unwrap();
        let entry1 = Entry::App("type".into(), "content 1".into());
        let entry1 = Entry::App("type".into(), "content 2".into());
        let header1 = bundle1.add_entry(&entry1).unwrap();

        let mut bundle2 = chain.bundle().unwrap();
        let entry2 = Entry::App("type".into(), "content 3".into());
        let entry2 = Entry::App("type".into(), "content 4".into());
        let header2 = bundle2.add_entry(&entry1).unwrap();

        let commit_attempt_1 = chain.try_commit(bundle1);
        let new_chain_head = chain.head().unwrap();

        assert_eq!(commit_attempt_1, Ok(()));
        assert_eq!(*new_chain_head.address(), header1.address());

        let commit_attempt_2 = chain.try_commit(bundle2);

        // TODO: replace this assertion with the actual error issuing from multiple writes, once we know what it is
        assert_eq!(
            commit_attempt_2,
            Err(SourceChainError::HeadMismatch(
                post_init_head,
                new_chain_head
            ))
        );
        assert_eq!(*chain.head().unwrap().address(), header1.address());
    }
}
