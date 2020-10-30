use hdk3::prelude::*;

#[hdk_entry(
    id = "post",
    required_validations = 5,
    required_validation_type = "full"
)]
struct Post(String);

#[hdk_entry(
    id = "msg",
    required_validations = 5,
    required_validation_type = "sub_chain"
)]
struct Msg(String);

#[hdk_entry(
    id = "priv_msg",
    required_validations = 5,
    required_validation_type = "full",
    visibility = "private"
)]
struct PrivMsg(String);

entry_defs![Post::entry_def(), Msg::entry_def(), PrivMsg::entry_def()];

fn post() -> Post {
    Post("foo".into())
}

fn msg() -> Msg {
    Msg("hello".into())
}

fn priv_msg() -> PrivMsg {
    PrivMsg("Don't tell anyone".into())
}

#[hdk_extern]
fn create_entry(_: ()) -> ExternResult<HeaderHash> {
    Ok(create_entry!(post())?)
}

#[hdk_extern]
fn create_post(post: Post) -> ExternResult<HeaderHash> {
    Ok(create_entry!(&post)?)
}

#[hdk_extern]
fn get_entry(_: ()) -> ExternResult<GetOutput> {
    Ok(GetOutput::new(get!(hash_entry!(post())?)?))
}

#[hdk_extern]
fn create_msg(_: ()) -> ExternResult<HeaderHash> {
    Ok(create_entry!(msg())?)
}

#[hdk_extern]
fn create_priv_msg(_: ()) -> ExternResult<HeaderHash> {
    Ok(create_entry!(priv_msg())?)
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

#[hdk_extern]
fn my_activity(_: ()) -> ExternResult<AgentActivity> {
    let agent = agent_info!()?.agent_latest_pubkey;
    let query = QueryFilter::new();
    Ok(get_agent_activity(agent, query, ActivityRequest::Full)?)
}
