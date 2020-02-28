use super::{error::ChainInvalidReason, SourceChainCommitBundle, SourceChainSnapshot};
use crate::{
    agent::error::{SourceChainError, SourceChainResult},
    cell::Cell,
    txn::{
        source_chain,
        source_chain::{Cursor, CursorRw},
    }, state::source_chain::SourceChainBuffer,
};
use fallible_iterator::FallibleIterator;
use holochain_json_api::error::JsonError;
use holochain_persistence_api::{
    cas::content::Address,
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
use sx_state::{RkvEnv, buffer::StoreBuffer};

/// Interface to the source chain as accessed through persistent storage.
/// From a SourceChain, you can construct a SourceChainSnapshot to make queries,
/// or a SourceChainCommitBundle, to start a write transaction and potentially commit
/// the changes later with `try_commit`.
pub struct SourceChain<'env> {
    env: &'env RkvEnv,
}

impl<'env> SourceChain<'env> {
    pub(crate) fn new(env: &'env RkvEnv) -> Self {
        Self { env }
    }

    pub fn now(&self) -> SourceChainResult<SourceChainSnapshot> {
        let db = SourceChainBuffer::create(self.env)?;
        let head = db.chain_head()?.ok_or(SourceChainError::ChainEmpty)?;
        SourceChainSnapshot::new(&db, head)
    }

    pub fn as_at(&self, head: Address) -> SourceChainResult<SourceChainSnapshot> {
        let db = SourceChainBuffer::create(self.env)?;
        SourceChainSnapshot::new(&db, head)
    }

    pub fn bundle(&self) -> SourceChainResult<SourceChainCommitBundle> {
        let db = SourceChainBuffer::create(self.env)?;
        SourceChainCommitBundle::new(db)
    }

    pub fn validate(&self) -> SourceChainResult<()> {
        let _ = self.now()?;
        Ok(())
    }

    fn header_for_entry(
        &self,
        prev: Option<&Address>,
        entry: &Entry,
        provenances: &[Provenance],
        timestamp: Iso8601,
    ) -> SourceChainResult<ChainHeader> {
        if prev.is_none() && entry.entry_type() != EntryType::Dna {
            error!("Attempting to create header for non-Dna entry, but the chain is empty");
            return Err(SourceChainError::ChainEmpty);
        }
        let header = ChainHeader::new(
            entry.entry_type(),
            entry.address(),
            provenances,
            prev.cloned(),
            None, // TODO
            None, // TODO!!
            timestamp,
        );
        Ok(header)
    }

    /// Return the current chain top address. If no top is persisted, this is treated an error.
    pub fn head(&self) -> SourceChainResult<ChainTop> {
        self.head_inner()?.ok_or(SourceChainError::ChainEmpty)
    }

    /// TODO: rewrite once we have the multi-LMDB cursors sorted out, so that we can
    /// read the chain head from a different DB
    fn head_inner(&self) -> SourceChainResult<Option<Address>> {
        Ok(SourceChainBuffer::create(self.env)?.chain_head()?)
    }

    // pub fn initialize(&self, writer: CursorRw, dna: Dna, agent: AgentId) -> SourceChainResult<()> {
    //     let dna_entry = Entry::Dna(Box::new(dna));
    //     let dna_header = self.header_for_entry(
    //         None,
    //         &dna_entry,
    //         &[Provenance::new(agent.address(), Signature::fake())],
    //         chrono::Utc::now().timestamp().into(),
    //     )?;
    //     let head = dna_header.address();
    //     writer.add(&dna_entry)?;
    //     writer.add(&dna_header)?;
    //     writer.add(&head)?;

    //     let agent_entry = Entry::AgentId(agent.clone());
    //     let agent_header = self.header_for_entry(
    //         Some(&head),
    //         &agent_entry,
    //         &[Provenance::new(agent.address(), Signature::fake())],
    //         chrono::Utc::now().timestamp().into(),
    //     )?;
    //     let head = agent_header.address();

    //     writer.add(&agent_entry)?;
    //     writer.add(&agent_header)?;
    //     writer.add(&head)?;
    //     writer.commit()?;

    //     Ok(())
    // }

    pub fn dna(&self) -> SourceChainResult<Dna> {
        self.now()?.dna()
    }

    pub fn agent_id(&self) -> SourceChainResult<AgentId> {
        self.now()?.agent_id()
    }

    /// Use the SCHH to attempt to write a bundle of changes
    pub fn try_commit(&self, bundle: SourceChainCommitBundle) -> SourceChainResult<()> {
        unimplemented!();
        // let bundle_head = bundle.original_head();
        // let self_head = self.head()?;
        // let db = bundle.buffer();
        // if *bundle_head == self_head {
        //     let writer = self.env.write()?;
        //     db.finalize(&mut writer)?;
        //     writer.commit()?;
        //     Ok(())
        // } else {
        //     Err(SourceChainError::HeadMismatch(
        //         bundle_head.clone(),
        //         self_head,
        //     ))
        // }
    }
}

lazy_static! {
    static ref CHAIN_HEAD_ADDRESS: HashString = HashString::from("chain-head");
}

pub type ChainTop = Address;

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

    pub fn test_initialized_chain<'env>(
        agent_name: &str,
        env: &'env SourceChainPersistence,
    ) -> SourceChain<'env> {
        let dna: Dna = test_dna(agent_name);
        let agent = AgentId::generate_fake(agent_name);
        let id: CellId = (dna.address(), agent.clone());
        let chain = SourceChain::new(&env);
        let writer = env.create_cursor_rw().unwrap();
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
        let dna: Dna = test_dna("a");
        let agent = AgentId::generate_fake("a");
        let id: CellId = (dna.address(), agent.clone());
        let tmpdir = TempDir::new("skunkworx").unwrap();
        let env = SourceChainPersistence::test(tmpdir.path());
        let chain = SourceChain::new(&env);
        let writer = env.create_cursor_rw().unwrap();

        assert_eq!(chain.validate(), Err(SourceChainError::ChainEmpty));

        chain.initialize(writer, dna, agent).unwrap();

        assert!(chain.validate().is_ok());
    }

    #[test]
    fn chains_can_have_new_entries_committed_in_bundles() {
        let tmpdir = TempDir::new("skunkworx").unwrap();
        let env = SourceChainPersistence::test(tmpdir.path());
        let chain = test_initialized_chain("a", &env);
        let post_init_head = chain.head().unwrap();

        let mut bundle = chain.bundle().unwrap();
        let headers: Vec<ChainHeader> = [
            Entry::App("type".into(), "content 1".into()),
            Entry::App("type".into(), "content 2".into()),
            Entry::App("type".into(), "content 3".into()),
        ]
        .iter()
        .map(|entry| bundle.add_entry(&entry).unwrap())
        .collect();

        // See that the uncommitted new entries are accessible through the iterator
        assert!(bundle
            .snapshot()
            .unwrap()
            .iter_back()
            .find(|h| Ok(h.address() == headers[1].address()))
            .unwrap()
            .is_some());

        // But also see that the new entries aren't actually committed yet!
        assert_eq!(chain.now().unwrap().iter_back().count().unwrap(), 2);
        bundle.commit().unwrap();
        // Only now should they be
        assert_eq!(chain.now().unwrap().iter_back().count().unwrap(), 5);
    }

    #[test]
    fn chains_are_protected_from_concurrent_transactional_writes_aka_as_at() {
        let tmpdir = TempDir::new("skunkworx").unwrap();
        let env = SourceChainPersistence::test(tmpdir.path());
        let chain = test_initialized_chain("a", &env);
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

        assert!(commit_attempt_1.is_ok());
        assert_eq!(*new_chain_head.address(), header1.address());

        let commit_attempt_2 = chain.try_commit(bundle2);

        // TODO: replace this assertion with the actual error issuing from multiple writes, once we know what it is
        assert_eq!(
            commit_attempt_2.unwrap_err(),
            SourceChainError::HeadMismatch(
                post_init_head,
                new_chain_head
            )
        );
        assert_eq!(*chain.head().unwrap().address(), header1.address());
    }
}
