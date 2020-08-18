use super::state::{cascade::Cascade, element_buf::ElementBuf, metadata::MetadataBufT};
use crate::conductor::entry_def_store::EntryDefBuf;
use error::{PrevHeaderError, SysValidationError, SysValidationResult};
use fallible_iterator::FallibleIterator;
use holochain_keystore::{AgentPubKeyExt, Signature};
use holochain_types::{dna::Zomes, header::NewEntryHeaderRef, Entry, EntryHashed};
use holochain_zome_types::{
    element::SignedHeaderHashed,
    header::{AppEntryType, EntryType, EntryUpdate},
    link::LinkTag,
    Header,
};

pub use crate::core::state::source_chain::{SourceChainError, SourceChainResult};
pub use holo_hash::*;
pub use holochain_types::{
    element::{Element, ElementExt},
    HeaderHashed, Timestamp,
};

mod error;
#[cfg(test)]
mod tests;

/// 15mb limit on Entries due to websocket limits.
/// Consider splitting large entries up.
pub const MAX_ENTRY_SIZE: usize = 15_000_000;

/// 10kb limit on LinkTags.
pub const MAX_TAG_SIZE: usize = 10_000;

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
    )
    .await?;

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
pub async fn sys_validate_header(
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

/// Verify the signature for this header
pub async fn verify_header_signature(sig: &Signature, header: &Header) -> SysValidationResult<()> {
    if header.author().verify_signature(sig, header).await? {
        Ok(())
    } else {
        Err(SysValidationError::VerifySignature(
            sig.clone(),
            header.clone(),
        ))
    }
}

/// Verify the author key was valid at the time
/// of signing with dpki
/// TODO: This is just a stub until we have dpki.
pub async fn author_key_is_valid(_author: AgentPubKey) -> SysValidationResult<()> {
    Ok(())
}

/// Check if we are holding the previous header
/// in the element vault and metadata vault
/// and return the header
pub async fn check_and_get_prev_header(
    author: AgentPubKey,
    prev_header_hash: &HeaderHash,
    meta_vault: &impl MetadataBufT,
    element_vault: &ElementBuf<'_>,
) -> SysValidationResult<Option<Header>> {
    check_prev_header_in_metadata(author, prev_header_hash, meta_vault)?;

    // Check we are actually holding the previous header
    let prev_header = element_vault
        .get_header(prev_header_hash)
        .await?
        .ok_or(PrevHeaderError::MissingVault)?
        .into_header_and_signature()
        .0
        .into_content();

    // TODO: Check the op is integrated or is this redundant?
    // Maybe this should happen if it's not found?

    Ok(Some(prev_header))
}

/// Check the prev header is in the metadata
pub fn check_prev_header_in_metadata(
    author: AgentPubKey,
    prev_header_hash: &HeaderHash,
    meta_vault: &impl MetadataBufT,
) -> SysValidationResult<()> {
    meta_vault
        .get_activity(author)?
        .find(|activity| Ok(prev_header_hash == &activity.header_hash))?
        .ok_or(PrevHeaderError::MissingMeta)?;
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
                header.prev_header().ok_or(PrevHeaderError::MissingPrev)?;
                Ok(())
            } else {
                Err(PrevHeaderError::InvalidRoot.into())
            }
        }
    }
}

/// Check that Dna headers are only added to empty source chains
pub async fn check_valid_if_dna(
    header: &Header,
    meta_vault: &impl MetadataBufT,
) -> SysValidationResult<()> {
    match header {
        Header::Dna(_) => meta_vault
            .get_activity(header.author().clone())?
            .next()?
            .map_or(Ok(()), |_| Err(PrevHeaderError::InvalidRoot.into())),
        _ => Ok(()),
    }
}

/// Check if there are other headers at this
/// sequence number
pub async fn check_chain_rollback(
    _header: &Header,
    _meta_vault: &impl MetadataBufT,
    _element_vault: &ElementBuf<'_>,
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
        Err(PrevHeaderError::Timestamp.into())
    }
}

/// Check the previous header is one less then the current
pub fn check_prev_seq(header: &Header, prev_header: &Header) -> SysValidationResult<()> {
    let header_seq = header.header_seq();
    let prev_seq = prev_header.header_seq();
    if header_seq > 0 && prev_seq == header_seq - 1 {
        Ok(())
    } else {
        Err(PrevHeaderError::InvalidSeq(header_seq, prev_seq))?
    }
}

/// Check the entry variant matches the variant in the headers entry type
pub fn check_entry_type(entry_type: &EntryType, entry: &Entry) -> SysValidationResult<()> {
    match (entry_type, entry) {
        (EntryType::AgentPubKey, Entry::Agent(_)) => Ok(()),
        (EntryType::App(_), Entry::App(_)) => Ok(()),
        (EntryType::CapClaim, Entry::CapClaim(_)) => Ok(()),
        (EntryType::CapGrant, Entry::CapGrant(_)) => Ok(()),
        _ => Err(SysValidationError::EntryType),
    }
}

/// Check the AppEntryType is valid for the zome.
/// Check the EntryDefId and ZomeId are in range.
pub fn check_app_entry_type(
    entry_type: &AppEntryType,
    zomes: &Zomes,
    entry_defs: EntryDefBuf<'_>,
) -> SysValidationResult<()> {
    Ok(())
}

/// Check the app entry type isn't private for store entry
pub fn check_not_private(
    entry_type: &AppEntryType,
    zomes: &Zomes,
    entry_defs: EntryDefBuf<'_>,
) -> SysValidationResult<()> {
    Ok(())
}

/// Check the headers entry hash matches the hash of the entry
pub async fn check_entry_hash(hash: &EntryHash, entry: &Entry) -> SysValidationResult<()> {
    if *hash == EntryHash::with_data(entry).await {
        Ok(())
    } else {
        Err(SysValidationError::EntryHash)
    }
}

/// Check the header should have an entry.
/// Is either a EntryCreate or EntryUpdate
pub fn check_new_entry_header(header: &Header) -> SysValidationResult<()> {
    match header {
        Header::EntryCreate(_) | Header::EntryUpdate(_) => Ok(()),
        _ => Err(SysValidationError::NotNewEntry(header.clone())),
    }
}

/// Check the entry size is under the MAX_ENTRY_SIZE
// TODO: This could be bad if someone just keeps sending large entries.
// Getting the size of a large vec over and over might be a DDOS?
pub fn check_entry_size(entry: &Entry) -> SysValidationResult<()> {
    match entry {
        Entry::App(bytes) => {
            let size = std::mem::size_of_val(&bytes.bytes()[..]);
            if size < MAX_ENTRY_SIZE {
                Ok(())
            } else {
                Err(SysValidationError::EntryTooLarge(size, MAX_ENTRY_SIZE))
            }
        }
        // Other entry types are small
        _ => Ok(()),
    }
}

/// Check the link tag size is under the MAX_TAG_SIZE
// TODO: This could be bad if someone just keeps sending large tags.
// Getting the size of a large vec over and over might be a DDOS?
pub fn check_tag_size(tag: &LinkTag) -> SysValidationResult<()> {
    let size = std::mem::size_of_val(&tag.0[..]);
    if size < MAX_TAG_SIZE {
        Ok(())
    } else {
        Err(SysValidationError::TagTooLarge(size, MAX_TAG_SIZE))
    }
}

/// Check a EntryUpdate's entry schema is the same for
/// original and new entry.
/// If EntryType::App Check the EntryDefId, ZomeId and visibility match
pub fn check_update_reference(
    eu: &EntryUpdate,
    new_entry: &NewEntryHeaderRef<'_>,
) -> SysValidationResult<()> {
    if eu.entry_type == *new_entry.entry_type() {
        Ok(())
    } else {
        Err(SysValidationError::UpdateTypeMismatch(
            eu.entry_type.clone(),
            new_entry.entry_type().clone(),
        ))
    }
}

/// Check we are actually holding an entry
pub async fn check_holding_entry(
    hash: &EntryHash,
    element_vault: &ElementBuf<'_>,
) -> SysValidationResult<EntryHashed> {
    element_vault
        .get_entry(&hash)
        .await?
        .ok_or_else(|| SysValidationError::NotHoldingDep(hash.clone().into()))
}

/// Check we are actually holding an header
pub async fn check_holding_header(
    hash: &HeaderHash,
    element_vault: &ElementBuf<'_>,
) -> SysValidationResult<SignedHeaderHashed> {
    element_vault
        .get_header(&hash)
        .await?
        .ok_or_else(|| SysValidationError::NotHoldingDep(hash.clone().into()))
}

/// Check we are actually holding an element and the entry
pub async fn check_holding_element(
    hash: &HeaderHash,
    element_vault: &ElementBuf<'_>,
) -> SysValidationResult<Element> {
    let el = element_vault
        .get_element(&hash)
        .await?
        .ok_or_else(|| SysValidationError::NotHoldingDep(hash.clone().into()))?;
    el.entry()
        .as_option()
        .ok_or_else(|| SysValidationError::NotHoldingDep(hash.clone().into()))?;
    Ok(el)
}

/// Check that the entry exists on the dht
pub async fn check_entry_exists(
    entry_hash: EntryHash,
    cascade: &mut Cascade<'_, '_>,
) -> SysValidationResult<EntryHashed> {
    cascade
        .exists_entry(entry_hash.clone(), Default::default())
        .await?
        .ok_or_else(|| SysValidationError::DepMissingFromDht(entry_hash.into()))
}

/// Check that the element exists on the dht
pub async fn check_element_exists(
    hash: HeaderHash,
    cascade: &mut Cascade<'_, '_>,
) -> SysValidationResult<Element> {
    cascade
        .exists(hash.clone().into(), Default::default())
        .await?
        .ok_or_else(|| SysValidationError::DepMissingFromDht(hash.into()))
}
