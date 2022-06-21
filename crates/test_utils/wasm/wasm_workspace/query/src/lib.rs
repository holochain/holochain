use hdk::prelude::*;
use integrity::*;

mod integrity;

fn path(s: &str) -> ExternResult<EntryHash> {
    let path = Path::from(s).typed(LinkTypes::Query)?;
    path.ensure()?;
    path.path_entry_hash()
}

#[hdk_extern]
fn query(args: QueryFilter) -> ExternResult<Vec<Commit>> {
    hdk::prelude::query(args)
}

#[hdk_extern]
fn add_path(s: String) -> ExternResult<EntryHash> {
    path(&s)
}
