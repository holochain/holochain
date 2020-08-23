use hdk3::prelude::*;
mod countree;

entry_defs![countree::CounTree::entry_def()];

#[hdk_extern]
fn new(_: ()) -> ExternResult<HeaderHash> {
    countree::CounTree::new()
}

#[hdk_extern]
fn details(header_hash: HeaderHash) -> ExternResult<GetDetailsOutput> {
    countree::CounTree::details(header_hash)
}

#[hdk_extern]
fn inc(header_hash: HeaderHash) -> ExternResult<HeaderHash> {
    countree::CounTree::inc(header_hash)
}

#[hdk_extern]
fn dec(header_hash: HeaderHash) -> ExternResult<HeaderHash> {
    countree::CounTree::dec(header_hash)
}
