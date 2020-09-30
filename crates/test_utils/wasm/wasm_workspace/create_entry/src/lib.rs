use hdk3::prelude::*;

#[hdk_entry(id = "post", required_validations = 5)]
struct Post(String);

#[hdk_entry(id = "msg", required_validations = 5)]
struct Msg(String);

entry_defs![Post::entry_def(), Msg::entry_def()];

fn post() -> Post {
    Post("foo".into())
}

#[hdk_extern]
fn create_entry(_: ()) -> ExternResult<HeaderHash> {
    Ok(create_entry!(post())?)
}

#[hdk_extern]
fn get_entry(_: ()) -> ExternResult<GetOutput> {
    Ok(GetOutput::new(get!(hash_entry!(post())?)?))
}

#[hdk_extern]
fn validate_create_entry_post(
    validation_data: ValidateData,
) -> ExternResult<ValidateCallbackResult> {
    let element = validation_data.element;
    let r = match element.entry().to_app_option::<Post>() {
        Ok(Some(post)) if &post.0 == "Banana" => {
            ValidateCallbackResult::Invalid("No Bananas!".to_string())
        }
        _ => ValidateCallbackResult::Valid,
    };
    Ok(r)
}
