use hdk::prelude::*;
use integrity::LinkTypes;

mod integrity;

#[hdk_extern]
fn path_entry_hash(path_string: String) -> ExternResult<EntryHash> {
    Path::from(path_string).path_entry_hash()
}

#[hdk_extern]
fn exists(path_string: String) -> ExternResult<bool> {
    debug!(%path_string);
    let p = Path::from(path_string).typed(LinkTypes::Path)?;
    debug!(?p);
    p.exists()
}

#[hdk_extern]
fn ensure(path_string: String) -> ExternResult<()> {
    Path::from(path_string).typed(LinkTypes::Path)?.ensure()
}

#[hdk_extern]
fn delete_link(delete_link: ActionHash) -> ExternResult<ActionHash> {
    hdk::prelude::delete_link(delete_link)
}

#[hdk_extern]
fn children(path_string: String) -> ExternResult<Vec<Link>> {
    Path::from(path_string).typed(LinkTypes::Path)?.children()
}

#[hdk_extern]
fn children_details(path_string: String) -> ExternResult<LinkDetails> {
    Path::from(path_string)
        .typed(LinkTypes::Path)?
        .children_details()
}
