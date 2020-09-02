//! Functions for checking the presence of data
//! either being held locally or existing on the DHT
use super::*;
use crate::core::workflow::sys_validation_workflow::types::{CheckLevel, Dependency};
use holochain_p2p::HolochainP2pCell;

macro_rules! check_holding {
    ($f:ident, $($hash:expr),+ => $dep:ident, $($ws:expr),+ ) => {{
        match $f($($hash),+, $($ws),+).await {
            Err(SysValidationError::ValidationError(ValidationError::NotHoldingDep(_))) => (),
            Err(e) => return Err(e),
            Ok(e) => return Ok(Dependency::$dep(e)),
        }
    }};
}

macro_rules! check_holding_el {
    ($ws:expr, $f:ident, $($hash:expr),+) => {{
        check_holding!($f, $($hash),+ => Proof, &$ws.element_vault);
        check_holding!($f, $($hash),+ => Proof, &$ws.element_judged);
        check_holding!($f, $($hash),+ => AwaitingProof, &$ws.element_pending);
    }};
}

macro_rules! check_holding_entry {
    ($ws:expr, $f:ident, $($hash:expr),+) => {{
        check_holding!($f, $($hash),+ => Proof, &$ws.element_vault, &$ws.meta_vault);
        check_holding!($f, $($hash),+ => Proof, &$ws.element_judged, &$ws.meta_judged);
        check_holding!($f, $($hash),+ => AwaitingProof, &$ws.element_pending, &$ws.meta_pending);
    }};
}

macro_rules! check_holding_meta {
    ($ws:expr, $f:ident, $($hash:expr),+) => {
        check_holding!($f, $($hash),+ => Proof, &$ws.meta_vault);
        check_holding!($f, $($hash),+ => Proof, &$ws.meta_judged);
        check_holding!($f, $($hash),+ => AwaitingProof, &$ws.meta_pending);
    };
}

/// Check validated and integrated stores for a dependant op
pub async fn check_holding_entry_all(
    hash: &EntryHash,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    check_level: CheckLevel,
) -> SysValidationResult<Dependency<Element>> {
    match check_level {
        CheckLevel::Holding => check_holding_entry_inner(hash, workspace).await,
        CheckLevel::Dht => check_entry_exists(hash.clone(), workspace, network).await,
    }
}

async fn check_holding_entry_inner(
    hash: &EntryHash,
    workspace: &SysValidationWorkspace,
) -> SysValidationResult<Dependency<Element>> {
    check_holding_entry!(workspace, check_holding_entry, hash);
    Err(ValidationError::NotHoldingDep(hash.clone().into()).into())
}

/// Check validated and integrated stores for a dependant op
pub async fn check_holding_header_all(
    hash: &HeaderHash,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    check_level: CheckLevel,
) -> SysValidationResult<Dependency<SignedHeaderHashed>> {
    match check_level {
        CheckLevel::Holding => check_holding_header_inner(hash, workspace).await,
        CheckLevel::Dht => check_header_exists(hash.clone(), workspace, network).await,
    }
}
async fn check_holding_header_inner(
    hash: &HeaderHash,
    workspace: &SysValidationWorkspace,
) -> SysValidationResult<Dependency<SignedHeaderHashed>> {
    check_holding_el!(workspace, check_holding_header, hash);
    Err(ValidationError::NotHoldingDep(hash.clone().into()).into())
}

/// Check validated and integrated stores for a dependant op
pub async fn check_holding_element_all(
    hash: &HeaderHash,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    check_level: CheckLevel,
) -> SysValidationResult<Dependency<Element>> {
    match check_level {
        CheckLevel::Holding => check_holding_element_inner(hash, workspace).await,
        CheckLevel::Dht => check_element_exists(hash.clone(), workspace, network).await,
    }
}
async fn check_holding_element_inner(
    hash: &HeaderHash,
    workspace: &SysValidationWorkspace,
) -> SysValidationResult<Dependency<Element>> {
    check_holding_el!(workspace, check_holding_element, hash);
    Err(ValidationError::NotHoldingDep(hash.clone().into()).into())
}

/// Check if we are holding the previous header
/// in the element vault and metadata vault
/// and return the header
pub async fn check_holding_prev_header_all(
    author: &AgentPubKey,
    prev_header_hash: &HeaderHash,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    check_level: CheckLevel,
) -> SysValidationResult<Dependency<SignedHeaderHashed>> {
    match check_level {
        CheckLevel::Holding => {
            check_holding_prev_header_inner(author, prev_header_hash, workspace).await
        }
        CheckLevel::Dht => check_header_exists(prev_header_hash.clone(), workspace, network).await,
    }
}

async fn check_holding_prev_header_inner(
    author: &AgentPubKey,
    prev_header_hash: &HeaderHash,
    workspace: &SysValidationWorkspace,
) -> SysValidationResult<Dependency<SignedHeaderHashed>> {
    // Need to check these are both the same dependency type.
    // If either is AwaitingProof then the return type must also be etc.
    let dep = check_prev_header_in_metadata_all(author, prev_header_hash, workspace).await?;
    Ok(check_holding_header_inner(&prev_header_hash, &workspace)
        .await?
        .min(&dep))
}

/// Check if we are holding a header from a store entry op
pub async fn check_holding_store_entry_all(
    entry_hash: &EntryHash,
    header_hash: &HeaderHash,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    check_level: CheckLevel,
) -> SysValidationResult<Dependency<Element>> {
    match check_level {
        CheckLevel::Holding => {
            check_holding_store_entry_inner(entry_hash, header_hash, workspace).await
        }
        CheckLevel::Dht => check_element_exists(header_hash.clone(), workspace, network).await,
    }
}

async fn check_holding_store_entry_inner(
    entry_hash: &EntryHash,
    header_hash: &HeaderHash,
    workspace: &SysValidationWorkspace,
) -> SysValidationResult<Dependency<Element>> {
    // Need to check these are both the same dependency type.
    // If either is AwaitingProof then the return type must also be etc.
    let dep = check_header_in_metadata_all(entry_hash, header_hash, workspace).await?;
    Ok(check_holding_element_inner(&header_hash, &workspace)
        .await?
        .min(&dep))
}

/// Check if we are holding a header from a add link op
pub async fn check_holding_link_add_all(
    header_hash: &HeaderHash,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    check_level: CheckLevel,
) -> SysValidationResult<Dependency<SignedHeaderHashed>> {
    match check_level {
        CheckLevel::Holding => check_holding_link_add_inner(header_hash, workspace).await,
        CheckLevel::Dht => check_header_exists(header_hash.clone(), workspace, network).await,
    }
}

async fn check_holding_link_add_inner(
    header_hash: &HeaderHash,
    workspace: &SysValidationWorkspace,
) -> SysValidationResult<Dependency<SignedHeaderHashed>> {
    // Need to check these are both the same dependency type.
    // If either is AwaitingProof then the return type must also be etc.
    let dep = check_holding_header_inner(&header_hash, &workspace).await?;
    let meta_dep =
        check_link_in_metadata_all(dep.as_inner().header(), header_hash, workspace).await?;
    Ok(dep.min(&meta_dep))
}

/// Check the prev header is in the metadata
pub(super) async fn check_prev_header_in_metadata<P: PrefixType>(
    author: &AgentPubKey,
    prev_header_hash: &HeaderHash,
    meta_vault: &impl MetadataBufT<P>,
) -> SysValidationResult<()> {
    fresh_reader!(meta_vault.env(), |r| {
        meta_vault
            .get_activity(&r, author.clone())?
            .find(|activity| Ok(prev_header_hash == &activity.header_hash))?
            .ok_or_else(|| ValidationError::NotHoldingDep(prev_header_hash.clone().into()))?;
        Ok(())
    })
}

/// Check we are holding the header in the metadata
/// as a reference from the entry
pub(super) async fn check_header_in_metadata<P: PrefixType>(
    entry_hash: &EntryHash,
    header_hash: &HeaderHash,
    meta_vault: &impl MetadataBufT<P>,
) -> SysValidationResult<()> {
    fresh_reader!(meta_vault.env(), |r| {
        meta_vault
            .get_headers(&r, entry_hash.clone())?
            .find(|h| Ok(h.header_hash == *header_hash))?
            .ok_or_else(|| ValidationError::NotHoldingDep(header_hash.clone().into()))?;
        Ok(())
    })
}

/// Check we are holding the add link in the metadata
/// as a reference from the base entry
pub(super) async fn check_link_in_metadata<P: PrefixType>(
    link_add: &Header,
    link_add_hash: &HeaderHash,
    meta_vault: &impl MetadataBufT<P>,
) -> SysValidationResult<()> {
    // Check the header is a LinkAdd
    let link_add: LinkAdd = link_add
        .clone()
        .try_into()
        .map_err(|_| ValidationError::NotLinkAdd(link_add_hash.clone()))?;

    // Full key always returns just one link
    let link_key = LinkMetaKey::from((&link_add, link_add_hash));

    fresh_reader!(meta_vault.env(), |r| {
        meta_vault
            .get_links_all(&r, &link_key)?
            .next()?
            .ok_or_else(|| {
                SysValidationError::from(ValidationError::NotHoldingDep(
                    link_add_hash.clone().into(),
                ))
            })
    })?;
    // If the link is there we no the link add is in the metadata
    Ok(())
}

/// Check the prev header is in the metadata
async fn check_prev_header_in_metadata_all(
    author: &AgentPubKey,
    prev_header_hash: &HeaderHash,
    workspace: &SysValidationWorkspace,
) -> SysValidationResult<Dependency<()>> {
    check_holding_meta!(
        workspace,
        check_prev_header_in_metadata,
        author,
        prev_header_hash
    );
    Err(ValidationError::NotHoldingDep(prev_header_hash.clone().into()).into())
}

async fn check_header_in_metadata_all(
    entry_hash: &EntryHash,
    header_hash: &HeaderHash,
    workspace: &SysValidationWorkspace,
) -> SysValidationResult<Dependency<()>> {
    check_holding_meta!(workspace, check_header_in_metadata, entry_hash, header_hash);
    Err(ValidationError::NotHoldingDep(header_hash.clone().into()).into())
}

async fn check_link_in_metadata_all(
    link_add: &Header,
    header_hash: &HeaderHash,
    workspace: &SysValidationWorkspace,
) -> SysValidationResult<Dependency<()>> {
    check_holding_meta!(workspace, check_link_in_metadata, link_add, header_hash);
    Err(ValidationError::NotHoldingDep(header_hash.clone().into()).into())
}

/// Check we are actually holding an entry
async fn check_holding_entry<P: PrefixType>(
    hash: &EntryHash,
    element_vault: &ElementBuf<P>,
    meta_vault: &impl MetadataBufT<P>,
) -> SysValidationResult<Element> {
    let entry_header = fresh_reader!(meta_vault.env(), |r| {
        let eh = meta_vault
            .get_headers(&r, hash.clone())?
            .next()?
            .map(|h| h.header_hash)
            .ok_or_else(|| ValidationError::NotHoldingDep(hash.clone().into()))?;
        SysValidationResult::Ok(eh)
    })?;
    let r = element_vault.get_element(&entry_header).await;
    if let Err(e) = &r {
        dbg!(e);
    }
    let r = r?;
    r.ok_or_else(|| ValidationError::NotHoldingDep(hash.clone().into()).into())
}

/// Check we are actually holding an header
async fn check_holding_header<P: PrefixType>(
    hash: &HeaderHash,
    element_vault: &ElementBuf<P>,
) -> SysValidationResult<SignedHeaderHashed> {
    element_vault
        .get_header(&hash)
        .await?
        .ok_or_else(|| ValidationError::NotHoldingDep(hash.clone().into()).into())
}

/// Check we are actually holding an element and the entry
async fn check_holding_element<P: PrefixType>(
    hash: &HeaderHash,
    element_vault: &ElementBuf<P>,
) -> SysValidationResult<Element> {
    let el = dbg!(element_vault.get_element(&hash).await);
    if let Err(e) = &el {
        dbg!(e);
    }
    let el = el?;
    let el = el.ok_or_else(|| ValidationError::NotHoldingDep(hash.clone().into()))?;

    el.entry()
        .as_option()
        .ok_or_else(|| ValidationError::NotHoldingDep(hash.clone().into()))?;
    Ok(el)
}

/// Check that the entry exists on the dht
pub async fn check_entry_exists(
    entry_hash: EntryHash,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
) -> SysValidationResult<Dependency<Element>> {
    check_holding_entry!(workspace, check_holding_entry, &entry_hash);
    let mut cascade = workspace.cascade(network);
    let el = cascade
        .exists(entry_hash.clone().into(), Default::default())
        .await?
        .ok_or_else(|| ValidationError::DepMissingFromDht(entry_hash.into()))?;
    Ok(Dependency::Claim(el))
}

/// Check that the header exists on the dht
pub async fn check_header_exists(
    hash: HeaderHash,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
) -> SysValidationResult<Dependency<SignedHeaderHashed>> {
    check_holding_el!(workspace, check_holding_header, &hash);
    let mut cascade = workspace.cascade(network);
    let h = cascade
        .exists_header(hash.clone(), Default::default())
        .await?
        .ok_or_else(|| ValidationError::DepMissingFromDht(hash.into()))?;
    Ok(Dependency::Claim(h))
}

/// Check that the element exists on the dht
pub async fn check_element_exists(
    hash: HeaderHash,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
) -> SysValidationResult<Dependency<Element>> {
    check_holding_el!(workspace, check_holding_element, &hash);
    let mut cascade = workspace.cascade(network);
    let el = cascade
        .exists(hash.clone().into(), Default::default())
        .await?
        .ok_or_else(|| ValidationError::DepMissingFromDht(hash.into()))?;
    Ok(Dependency::Claim(el))
}
