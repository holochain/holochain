use hdk::prelude::*;

entry_defs![Path::entry_def()];

#[hdk_extern]
fn hash(path_string: String) -> ExternResult<EntryHash> {
    Path::from(path_string).hash()
}

#[hdk_extern]
fn exists(path_string: String) -> ExternResult<bool> {
    Path::from(path_string).exists()
}

#[hdk_extern]
fn ensure(path_string: String) -> ExternResult<()> {
    Path::from(path_string).ensure()
}

#[hdk_extern]
fn delete_link(delete_link: HeaderHash) -> ExternResult<HeaderHash> {
    hdk::prelude::delete_link(delete_link)
}

#[hdk_extern]
fn children(path_string: String) -> ExternResult<Links> {
    Path::from(path_string).children()
}

#[hdk_extern]
fn children_details(path_string: String) -> ExternResult<LinkDetails> {
    Path::from(path_string).children_details()
}
