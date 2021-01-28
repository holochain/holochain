use hdk3::prelude::*;

entry_defs![Path::entry_def()];

fn path(s: &str) -> ExternResult<EntryHash> {
    let path = Path::from(s);
    path.ensure()?;
    path.hash()
}

#[hdk_extern]
fn query(args: QueryFilter) -> ExternResult<ElementVec> {
    hdk3::prelude::query(args)
}

#[hdk_extern]
fn add_path(s: String) -> ExternResult<EntryHash> {
    path(&s)
}
