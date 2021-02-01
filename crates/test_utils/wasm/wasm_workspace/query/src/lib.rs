use hdk3::prelude::*;

entry_defs![Path::entry_def()];

#[derive(Serialize, Deserialize, SerializedBytes)]
struct PathString(String);

fn path(s: &str) -> ExternResult<EntryHash> {
    let path = Path::from(s);
    path.ensure()?;
    Ok(path.hash()?)
}

#[hdk_extern]
fn query(args: QueryFilter) -> ExternResult<ElementVec> {
    Ok(hdk3::prelude::query(args)?)
}

#[hdk_extern]
fn add_path(s: PathString) -> ExternResult<EntryHash> {
    path(&s.0)
}
