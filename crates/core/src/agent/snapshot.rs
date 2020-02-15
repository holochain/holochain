use crate::{
    agent::{
        error::{ChainInvalidReason, SourceChainError, SourceChainResult},
        ChainTop,
    },
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
use std::fmt;
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
    pub(super) fn new(reader: source_chain::Cursor, head: ChainTop) -> SourceChainResult<Self> {
        match reader.contains(head.address()) {
            Ok(true) => Ok(Self { reader, head }),
            Ok(false) => Err(SourceChainError::MissingHead),
            Err(e) => Err(SkunkError::from(e).into()),
        }
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

pub struct SourceChainCommitBundle {
    writer: source_chain::CursorRw,
    original_head: ChainTop,
    new_head: ChainTop,
}

impl SourceChainCommitBundle {
    pub(super) fn new(writer: source_chain::CursorRw, head: ChainTop) -> SourceChainResult<Self> {
        // Just ensure that a snapshot can be created, mainly to perform the chain head integrity check
        let _ = SourceChainSnapshot::new(writer.clone(), head.clone())?;
        Ok(Self {
            writer,
            original_head: head.clone(),
            new_head: head,
        })
    }

    pub fn add_entry(&mut self, entry: &Entry) -> SourceChainResult<ChainHeader> {
        self.writer.add(entry)?;
        let header = self.header_for_entry(entry)?;
        self.writer.add(&header)?;
        self.new_head = ChainTop::new(header.address());
        self.writer.add(&self.new_head)?; // update the chain top
        Ok(header)
    }

    pub fn original_head(&self) -> &ChainTop {
        &self.original_head
    }


    pub fn commit(self) -> SourceChainResult<()> {
        Ok(self.writer.commit()?)
    }

    pub fn readonly_cursor(&self) -> source_chain::Cursor {
        self.writer.clone()
    }

    fn header_for_entry(&self, entry: &Entry) -> SourceChainResult<ChainHeader> {
        let provenances = &[Provenance::new(
            self.snapshot()?.agent_id().unwrap().address(),
            Signature::fake(),
        )];
        let timestamp = chrono::Utc::now().timestamp().into();
        let header = ChainHeader::new(
            entry.entry_type(),
            entry.address(),
            provenances,
            Some(self.new_head.address().clone()),
            None,
            None,
            timestamp,
        );
        Ok(header)
    }

    pub fn snapshot(&self) -> SourceChainResult<SourceChainSnapshot> {
        SourceChainSnapshot::new(self.readonly_cursor(), self.new_head.clone())
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
                    self.current = header.link().map(ChainTop::new);
                    Ok(Some(header))
                } else {
                    Ok(None)
                }
            }
        }
    }
}

impl fmt::Debug for SourceChainSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut iter = self.iter_back();
        while let Some(header) = iter.next().map_err(|_| fmt::Error)? {
            write!(f, "{}\n", header.address())?;
        }
        Ok(())
    }
}
