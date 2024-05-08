use crate::integrity::*;
use hdk::prelude::*;

#[hdk_extern]
pub fn create_post(post: Post) -> ExternResult<Record> {
    let post_hash = create_entry(&EntryTypes::Post(post.clone()))?;
    let record = get(post_hash.clone(), GetOptions::default())?.ok_or(wasm_error!(
        WasmErrorInner::Guest("Could not find the newly created Post".to_string())
    ))?;
    let path = Path::from("all_posts");
    create_link(
        path.path_entry_hash()?,
        post_hash.clone(),
        LinkTypes::AllPosts,
        (),
    )?;
    let path = Path::from("some_other_path");
    create_link(
        path.path_entry_hash()?,
        post_hash.clone(),
        LinkTypes::AllPosts,
        (),
    )?;
    let path = Path::from("yet_another_path");
    create_link(
        path.path_entry_hash()?,
        post_hash.clone(),
        LinkTypes::AllPosts,
        (),
    )?;
    let path = Path::from("yap");
    create_link(
        path.path_entry_hash()?,
        post_hash.clone(),
        LinkTypes::AllPosts,
        (),
    )?;
    let my_agent_pub_key = agent_info()?.agent_latest_pubkey;
    create_link(
        my_agent_pub_key,
        post_hash.clone(),
        LinkTypes::PostsByAuthor,
        (),
    )?;
    Ok(record)
}

#[hdk_extern]
pub fn get_all_posts() -> ExternResult<Vec<Link>> {
    let path = Path::from("all_posts");
    get_links(GetLinksInputBuilder::try_new(path.path_entry_hash()?, LinkTypes::AllPosts)?.build())
}
