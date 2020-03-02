use crate::{
    agent::{
        error::{ChainInvalidReason, SourceChainError, SourceChainResult},
    },
    cell::Cell,
    state::{chain_cas::HeaderCas, source_chain::SourceChainBuffer},
    txn::{
        source_chain,
        source_chain::{Cursor, CursorRw},
    },
};
use core::ops::Deref;
use fallible_iterator::FallibleIterator;
use holochain_json_api::error::JsonError;
use lazy_static::*;
use std::{borrow::Borrow, fmt, rc::Rc, cell::{Ref, RefCell}};
use sx_state::{Reader, RkvEnv, Writer};
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
use owning_ref::BoxRef;

pub type SourceChainSnapshot<'env> = SourceChainSnapshotAbstract<'env, SourceChainBuffer<'env>>;
pub type SourceChainSnapshotRef<'env> =
    SourceChainSnapshotAbstract<'env, &'env SourceChainBuffer<'env>>;
pub type SourceChainSnapshotRefRef<'env> =
    SourceChainSnapshotAbstract<'env, Ref<'env, SourceChainBuffer<'env>>>;


/// Representation of a Cell's source chain.
/// TODO: work out the details of what's needed for as-at
/// to make sure the right balance is struck between
/// creating as-at snapshots and having access to the actual current source chain
pub struct SourceChainSnapshotAbstract<'env, Db: Borrow<SourceChainBuffer<'env>>> {
    db: Db,
    head: Address,
    _lifetime: std::marker::PhantomData<&'env ()>,
}

impl<'env, Db: Borrow<SourceChainBuffer<'env>>> SourceChainSnapshotAbstract<'env, Db> {
    /// Fails if a source chain has not yet been created for this CellId.
    pub(super) fn new(db: Db, head: Address) -> SourceChainResult<Self> {
        if db.borrow().get_header(&head)?.is_some() {
            Ok(Self {
                db,
                head,
                _lifetime: std::marker::PhantomData,
            })
        } else {
            Err(SourceChainError::MissingHead)
        }
    }

    pub fn head(&self) -> &Address {
        &self.head
    }

    pub fn dna(&self) -> SourceChainResult<Dna> {
        let entry = self.latest_entry_of_type(EntryType::Dna)?.ok_or(
            SourceChainError::InvalidStructure(ChainInvalidReason::GenesisMissing),
        )?;
        if let Entry::Dna(dna) = entry {
            Ok(*dna)
        } else {
            Err(SourceChainError::InvalidStructure(
                ChainInvalidReason::HeaderAndEntryMismatch(entry.address()),
            ))
        }
    }

    pub fn agent_id(&self) -> SourceChainResult<AgentId> {
        let entry = self.latest_entry_of_type(EntryType::AgentId)?.ok_or(
            SourceChainError::InvalidStructure(ChainInvalidReason::GenesisMissing),
        )?;
        if let Entry::AgentId(agent) = entry {
            Ok(agent)
        } else {
            Err(SourceChainError::InvalidStructure(
                ChainInvalidReason::HeaderAndEntryMismatch(entry.address()),
            ))
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
            headers: self.db.borrow().headers(),
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
            Ok(self
                .db
                .borrow()
                .cas()
                .header_with_entry(header)?
                .map(|hwe| hwe.entry().clone()))
        } else {
            Ok(None)
        }
    }
}

pub struct SourceChainCommitBundle<'env> {
    db: RefCell<SourceChainBuffer<'env>>,
    // snapshot: SourceChainSnapshotRefRef<'env>,
    original_head: Address,
    new_head: Address,
}

impl<'env> SourceChainCommitBundle<'env> {
    pub(super) fn new(db: SourceChainBuffer<'env>) -> SourceChainResult<Self> {
        let head = db.chain_head()?.ok_or(SourceChainError::ChainEmpty)?;
        let _ = SourceChainSnapshotRef::new(&db, head.clone())?;
        let db = RefCell::new(db);
        Ok(Self {
            db: db,
            // snapshot,
            original_head: head.clone(),
            new_head: head,
        })
    }

    pub fn add_entry(&mut self, entry: Entry) -> SourceChainResult<ChainHeader> {
        let header = self.header_for_entry(&entry)?;
        self.db.borrow_mut().put((header.clone(), entry));
        Ok(header)
    }

    pub fn original_head(&self) -> &Address {
        &self.original_head
    }

    /// Extract the underlying buffer which has been filled with staged changes,
    /// consuming the outer struct. This gets passed to SourceChain::try_commit.
    pub fn buffer(self) -> SourceChainBuffer<'env> {
        self.db.into_inner()
    }

    pub fn snapshot(&self) -> SourceChainSnapshotRef<'env> {
        // &self.snapshot
        SourceChainSnapshotRef::new(&*self.db.borrow(), self.head.clone())
    }

    fn header_for_entry(&self, entry: &Entry) -> SourceChainResult<ChainHeader> {
        let provenances = &[Provenance::new(
            self.snapshot.agent_id().unwrap().address(),
            Signature::fake(),
        )];
        let timestamp = chrono::Utc::now().timestamp().into();
        let header = ChainHeader::new(
            entry.entry_type(),
            entry.address(),
            provenances,
            Some(self.new_head.clone()),
            None,
            None,
            timestamp,
        );
        Ok(header)
    }
}

pub struct SourceChainBackwardIterator<'env> {
    headers: &'env HeaderCas<'env>,
    current: Option<Address>,
}

/// Follows ChainHeader.link through every previous Entry (of any EntryType) in the chain
// #[holochain_tracing_macros::newrelic_autotrace(HOLOCHAIN_CORE)]
impl<'env> FallibleIterator for SourceChainBackwardIterator<'env> {
    type Item = ChainHeader;
    type Error = SourceChainError;

    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        match &self.current {
            None => Ok(None),
            Some(head) => {
                if let Some(header) = self.headers.get(head)? {
                    self.current = header.link();
                    Ok(Some(header))
                } else {
                    Ok(None)
                }
            }
        }
    }
}

impl<'env> fmt::Debug for SourceChainSnapshot<'env> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut iter = self.iter_back();
        while let Some(header) = iter.next().map_err(|_| fmt::Error)? {
            write!(f, "{}\n", header.address())?;
        }
        Ok(())
    }
}
