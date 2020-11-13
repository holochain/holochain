use hdk3::prelude::*;
mod countree;

entry_defs![countree::CounTree::entry_def()];

#[hdk_extern]
fn new(_: ()) -> ExternResult<HeaderHash> {
    countree::CounTree::new()
}

#[hdk_extern]
fn header_details(header_hash: HeaderHash) -> ExternResult<GetDetailsOutput> {
    countree::CounTree::header_details(header_hash)
}

#[hdk_extern]
fn entry_details(entry_hash: EntryHash) -> ExternResult<GetDetailsOutput> {
    countree::CounTree::entry_details(entry_hash)
}

#[hdk_extern]
fn entry_hash(countree: countree::CounTree) -> ExternResult<EntryHash> {
    Ok(hash_entry(&countree)?)
}

#[hdk_extern]
fn inc(header_hash: HeaderHash) -> ExternResult<HeaderHash> {
    countree::CounTree::incsert(header_hash)
}

#[hdk_extern]
fn dec(header_hash: HeaderHash) -> ExternResult<HeaderHash> {
    countree::CounTree::dec(header_hash)
}
