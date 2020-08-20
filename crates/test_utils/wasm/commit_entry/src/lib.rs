use hdk3::prelude::*;

const POST_ID: &str = "post";
#[derive(Default, SerializedBytes, serde::Serialize, serde::Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
struct Post(String);

entry_defs!(
    def Post EntryDef {
        id: POST_ID.into(),
        ..Default::default()
    };
);

fn post() -> Post {
    Post("foo".into())
}

#[hdk(extern)]
fn commit_entry(_: ()) -> ExternResult<HeaderHash> {
    Ok(commit_entry!(post())?)
}

#[hdk(extern)]
fn get_entry(_: ()) -> ExternResult<GetOutput> {
    Ok(GetOutput::new(get!(entry_hash!(post())?)?))
}
