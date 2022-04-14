use holochain_deterministic_integrity::prelude::*;

#[hdk_entry(
    id = "post",
    required_validations = 5,
    required_validation_type = "full"
)]
pub struct Post(pub String);

#[hdk_entry(
    id = "msg",
    required_validations = 5,
    required_validation_type = "sub_chain"
)]
pub struct Msg(pub String);

#[hdk_entry(
    id = "priv_msg",
    required_validations = 5,
    required_validation_type = "full",
    visibility = "private"
)]
pub struct PrivMsg(pub String);

entry_defs![Post::entry_def(), Msg::entry_def(), PrivMsg::entry_def()];

#[hdk_extern]
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    let this_zome = zome_info()?;
    if let Op::StoreEntry {
        header:
            SignedHashed {
                hashed: HoloHashed {
                    content: header, ..
                },
                ..
            },
        entry,
    } = op
    {
        if header
            .app_entry_type()
            .filter(|app_entry_type| {
                this_zome.matches_entry_def_id(app_entry_type, Post::entry_def_id())
            })
            .map_or(Ok(false), |_| {
                Post::try_from(entry).map(|post| &post.0 == "Banana")
            })?
        {
            return Ok(ValidateCallbackResult::Invalid("No Bananas!".to_string()));
        }
    }
    Ok(ValidateCallbackResult::Valid)
}
