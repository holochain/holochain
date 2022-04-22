use hdk::prelude::*;

#[derive(ToZomeName)]
enum Zomes {
    IntegrityQuery,
}

fn path(s: &str) -> ExternResult<EntryHash> {
    let path = Path::from(s).locate(Zomes::IntegrityQuery);
    path.ensure()?;
    path.path_entry_hash()
}

#[hdk_extern]
fn query(args: QueryFilter) -> ExternResult<Vec<Element>> {
    hdk::prelude::query(args)
}

#[hdk_extern]
fn add_path(s: String) -> ExternResult<EntryHash> {
    path(&s)
}
