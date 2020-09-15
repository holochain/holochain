//! # System Validation Checks
//! This module contains all the checks we run for sys validation

use super::{
    state::{
        element_buf::ElementBuf,
        metadata::{LinkMetaKey, MetadataBufT},
    },
    workflow::sys_validation_workflow::SysValidationWorkspace,
};
use crate::conductor::{
    api::CellConductorApiT,
    entry_def_store::get_entry_def,
    entry_def_store::{get_entry_defs, EntryDefBufferKey},
};
use fallible_iterator::FallibleIterator;
use holochain_keystore::{AgentPubKeyExt, Signature};
use holochain_state::{fresh_reader, prelude::PrefixType};
use holochain_types::{header::NewEntryHeaderRef, Entry};
use holochain_zome_types::{
    element::SignedHeaderHashed,
    entry_def::{EntryDef, EntryVisibility},
    header::{AppEntryType, EntryType, EntryUpdate, LinkAdd},
    link::LinkTag,
    Header,
};
use std::convert::TryInto;

pub use crate::core::state::source_chain::{SourceChainError, SourceChainResult};
pub(super) use error::ValidationOutcome;
pub(super) use error::{PrevHeaderError, SysValidationError, SysValidationResult};
pub use holo_hash::*;
pub use holochain_types::{
    element::{Element, ElementExt},
    HeaderHashed, Timestamp,
};

pub use present::*;

#[allow(missing_docs)]
mod error;
mod present;
#[cfg(test)]
mod tests;

/// 16mb limit on Entries due to websocket limits.
/// Consider splitting large entries up.
pub const MAX_ENTRY_SIZE: usize = 16_000_000;

/// 400b limit on LinkTags.
/// Tags are used as keys to the database to allow
/// fast lookup so they need to be small.
pub const MAX_TAG_SIZE: usize = 400;

/////////////
// TODO: These checks are old and should probably be removed when
// we implement the direct sys validation call
/////////////

/// Ensure that a given pre-fetched element is actually valid on this chain.
///
/// Namely:
/// - The header signature is valid.
/// - The header is valid (see validate_header).
/// - The signature was authored by the agent that owns this chain.
/// - @TODO - The entry content hashes properly & matches the hash in the header.
/// - @TODO - The entry content is shaped properly according to the header type.
/// - @TODO - The serialized entry content is < 100MB.
pub async fn sys_validate_element(
    author: &AgentPubKey,
    element: &Element,
    prev_element: Option<&Element>,
) -> SourceChainResult<()> {
    // The header signature is valid.
    element.validate().await?;

    // The header is valid.
    sys_validate_header(
        element.header_hashed(),
        prev_element.map(|e| e.header_hashed()),
    )?;

    // The header was authored by the agent that owns this chain.
    if element.header().author() != author {
        tracing::error!(
            "Author mismatch! {} != {}, element: {:?}",
            element.header().author(),
            author,
            element
        );
        return Err(SourceChainError::InvalidSignature);
    }

    // - @TODO - The entry content hashes properly & matches the hash in the header.

    // - @TODO - The entry content is shaped properly according to the header type.

    // - @TODO - The serialized entry content is < 100MB.

    Ok(())
}

/// Ensure that a given pre-fetched header is actually valid on this chain.
///
/// Namely:
/// - If the header type contains a previous header reference
///   (true for everything except the Dna header).
///   Then, ensure the previous header timestamp sequence /
///   ordering is correct, and the previous header is strictly the previous
///   header by sequence.
/// - @TODO - The agent was valid in DPKI at time of signing.
pub fn sys_validate_header(
    header: &HeaderHashed,
    prev_header: Option<&HeaderHashed>,
) -> SourceChainResult<()> {
    // - If the header type contains a previous header reference
    //   (true for everything except the Dna header).
    //   Then, ensure the previous header timestamp sequence /
    //   ordering is correct, and the previous header is strictly the previous
    //   header by sequence.

    // the only way this can be None is for Dna,
    // in the case of Dna, we don't need to check the previous header.
    if let Some(asserted_prev_header) = header.prev_header() {
        // verify we have the correct previous header
        let prev_header = match prev_header {
            None => {
                return Err(SourceChainError::InvalidPreviousHeader(
                    "expected previous header, received None".to_string(),
                ));
            }
            Some(prev_header) => prev_header,
        };

        // ensure the hashes match
        if asserted_prev_header != prev_header.as_hash() {
            return Err(SourceChainError::InvalidPreviousHeader(format!(
                "expected header hash: {}, received: {}",
                asserted_prev_header,
                prev_header.as_hash(),
            )));
        }

        // make sure the timestamps are in order
        if header.timestamp() < prev_header.timestamp() {
            return Err(SourceChainError::InvalidPreviousHeader(format!(
                "expected timestamp < {}, received: {}",
                Timestamp::from(header.timestamp()).to_string(),
                Timestamp::from(prev_header.timestamp()).to_string(),
            )));
        }

        // make sure the header_seq is strictly ordered
        if header.header_seq() - 1 != prev_header.header_seq() {
            return Err(SourceChainError::InvalidPreviousHeader(format!(
                "expected header_seq: {}, received: {}",
                header.header_seq() - 1,
                prev_header.header_seq(),
            )));
        }
    }

    // - @TODO - The agent was valid in DPKI at time of signing.

    Ok(())
}

///////////////////////////////
// Sys validation starts here
//////////////////////////////

/// Verify the signature for this header
pub async fn verify_header_signature(sig: &Signature, header: &Header) -> SysValidationResult<()> {
    if header.author().verify_signature(sig, header).await? {
        Ok(())
    } else {
        Err(ValidationOutcome::VerifySignature(sig.clone(), header.clone()).into())
    }
}

/// Verify the author key was valid at the time
/// of signing with dpki
/// TODO: This is just a stub until we have dpki.
pub async fn author_key_is_valid(_author: &AgentPubKey) -> SysValidationResult<()> {
    Ok(())
}

/// Check that previous header makes sense
/// for this header.
/// If not Dna then cannot be root of chain
/// and must have previous header
pub fn check_prev_header(header: &Header) -> SysValidationResult<()> {
    match &header {
        Header::Dna(_) => Ok(()),
        _ => {
            if header.header_seq() > 0 {
                header
                    .prev_header()
                    .ok_or(PrevHeaderError::MissingPrev)
                    .map_err(ValidationOutcome::from)?;
                Ok(())
            } else {
                Err(PrevHeaderError::InvalidRoot).map_err(|e| ValidationOutcome::from(e).into())
            }
        }
    }
}

/// Check that Dna headers are only added to empty source chains
pub async fn check_valid_if_dna(
    header: &Header,
    meta_vault: &impl MetadataBufT,
) -> SysValidationResult<()> {
    fresh_reader!(meta_vault.env(), |r| {
        match header {
            Header::Dna(_) => meta_vault
                .get_activity(&r, header.author().clone())?
                .next()?
                .map_or(Ok(()), |_| {
                    Err(PrevHeaderError::InvalidRoot).map_err(|e| ValidationOutcome::from(e).into())
                }),
            _ => Ok(()),
        }
    })
}

/// Check if there are other headers at this
/// sequence number
pub async fn check_chain_rollback(
    _header: &Header,
    _meta_vault: &impl MetadataBufT,
    _element_vault: &ElementBuf,
) -> SysValidationResult<()> {
    // Will need to pull out all headers to check this.
    // TODO: Do we need some way of storing headers by
    // sequence number in the metadata store?
    Ok(())
}

/// Placeholder for future spam check.
/// Check header timestamps don't exceed MAX_PUBLISH_FREQUENCY
pub async fn check_spam(_header: &Header) -> SysValidationResult<()> {
    Ok(())
}

/// Check previous header timestamp is before this header
pub fn check_prev_timestamp(header: &Header, prev_header: &Header) -> SysValidationResult<()> {
    if header.timestamp() > prev_header.timestamp() {
        Ok(())
    } else {
        Err(PrevHeaderError::Timestamp).map_err(|e| ValidationOutcome::from(e).into())
    }
}

/// Check the previous header is one less then the current
pub fn check_prev_seq(header: &Header, prev_header: &Header) -> SysValidationResult<()> {
    let header_seq = header.header_seq();
    let prev_seq = prev_header.header_seq();
    if header_seq > 0 && prev_seq == header_seq - 1 {
        Ok(())
    } else {
        Err(PrevHeaderError::InvalidSeq(header_seq, prev_seq))
            .map_err(|e| ValidationOutcome::from(e).into())
    }
}

/// Check the entry variant matches the variant in the headers entry type
pub fn check_entry_type(entry_type: &EntryType, entry: &Entry) -> SysValidationResult<()> {
    match (entry_type, entry) {
        (EntryType::AgentPubKey, Entry::Agent(_)) => Ok(()),
        (EntryType::App(_), Entry::App(_)) => Ok(()),
        (EntryType::CapClaim, Entry::CapClaim(_)) => Ok(()),
        (EntryType::CapGrant, Entry::CapGrant(_)) => Ok(()),
        _ => Err(ValidationOutcome::EntryType.into()),
    }
}

/// Check the AppEntryType is valid for the zome.
/// Check the EntryDefId and ZomeId are in range.
pub async fn check_app_entry_type(
    entry_type: &AppEntryType,
    conductor_api: &impl CellConductorApiT,
) -> SysValidationResult<EntryDef> {
    let zome_index = u8::from(entry_type.zome_id()) as usize;
    // We want to be careful about holding locks open to the conductor api
    // so calls are made in blocks
    let dna_file = { conductor_api.get_this_dna().await };
    let dna_file =
        dna_file.ok_or_else(|| SysValidationError::DnaMissing(conductor_api.cell_id().clone()))?;

    // Check if the zome is found
    let zome = dna_file
        .dna()
        .zomes
        .get(zome_index)
        .ok_or_else(|| ValidationOutcome::ZomeId(entry_type.clone()))?
        .1
        .clone();

    let entry_def = get_entry_def(entry_type, zome, &dna_file, conductor_api).await?;

    // Check the visibility and return
    match entry_def {
        Some(entry_def) => {
            if entry_def.visibility == *entry_type.visibility() {
                Ok(entry_def)
            } else {
                Err(ValidationOutcome::EntryVisibility(entry_type.clone()).into())
            }
        }
        None => Err(ValidationOutcome::EntryDefId(entry_type.clone()).into()),
    }
}

/// Check the app entry type isn't private for store entry
pub fn check_not_private(entry_def: &EntryDef) -> SysValidationResult<()> {
    match entry_def.visibility {
        EntryVisibility::Public => Ok(()),
        EntryVisibility::Private => Err(ValidationOutcome::PrivateEntry.into()),
    }
}

/// Check the headers entry hash matches the hash of the entry
pub async fn check_entry_hash(hash: &EntryHash, entry: &Entry) -> SysValidationResult<()> {
    if *hash == EntryHash::with_data_sync(entry) {
        Ok(())
    } else {
        Err(ValidationOutcome::EntryHash.into())
    }
}

/// Check the header should have an entry.
/// Is either a EntryCreate or EntryUpdate
pub fn check_new_entry_header(header: &Header) -> SysValidationResult<()> {
    match header {
        Header::EntryCreate(_) | Header::EntryUpdate(_) => Ok(()),
        _ => Err(ValidationOutcome::NotNewEntry(header.clone()).into()),
    }
}

/// Check the entry size is under the MAX_ENTRY_SIZE
pub fn check_entry_size(entry: &Entry) -> SysValidationResult<()> {
    match entry {
        Entry::App(bytes) => {
            let size = std::mem::size_of_val(&bytes.bytes()[..]);
            if size < MAX_ENTRY_SIZE {
                Ok(())
            } else {
                Err(ValidationOutcome::EntryTooLarge(size, MAX_ENTRY_SIZE).into())
            }
        }
        // Other entry types are small
        _ => Ok(()),
    }
}

/// Check the link tag size is under the MAX_TAG_SIZE
pub fn check_tag_size(tag: &LinkTag) -> SysValidationResult<()> {
    let size = std::mem::size_of_val(&tag.0[..]);
    if size < MAX_TAG_SIZE {
        Ok(())
    } else {
        Err(ValidationOutcome::TagTooLarge(size, MAX_TAG_SIZE).into())
    }
}

/// Check a EntryUpdate's entry type is the same for
/// original and new entry.
pub fn check_update_reference(
    eu: &EntryUpdate,
    original_entry_header: &NewEntryHeaderRef<'_>,
) -> SysValidationResult<()> {
    if eu.entry_type == *original_entry_header.entry_type() {
        Ok(())
    } else {
        Err(ValidationOutcome::UpdateTypeMismatch(
            eu.entry_type.clone(),
            original_entry_header.entry_type().clone(),
        )
        .into())
    }
}
