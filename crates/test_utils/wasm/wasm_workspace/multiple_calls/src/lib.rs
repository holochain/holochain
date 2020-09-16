use hdk3::prelude::*;

#[hdk_entry(id = "post", required_validations = 5)]
struct Post(String);

entry_defs![Post::entry_def()];

fn post() -> Post {
    Post("foo".into())
}

#[hdk_extern]
fn create_entry_multiple(_: ()) -> ExternResult<HeaderHash> {
    for _ in 0..140 {
        create_entry!(post())?;
    }

    Ok(create_entry!(post())?)
}

#[hdk_extern]
fn get_entry_multiple(_: ()) -> ExternResult<GetOutput> {
    let address = hash_entry!(post())?;
    for _ in 0..250 {
        get!(address.clone())?;
    }

    Ok(GetOutput::new(get!(address)?))
}
