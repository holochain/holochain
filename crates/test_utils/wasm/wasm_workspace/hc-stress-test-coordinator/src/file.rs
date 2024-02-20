use hdk::prelude::*;
use files_integrity::*;
#[hdk_extern]
pub fn create_file(file: File) -> ExternResult<Record> {
    let file_hash = create_entry(&EntryTypes::File(file.clone()))?;
    let record = get(file_hash.clone(), GetOptions::default())?
        .ok_or(
            wasm_error!(
                WasmErrorInner::Guest(String::from("Could not find the newly created File"))
            ),
        )?;
    let path = Path::from("all_images");
    create_link(path.path_entry_hash()?, file_hash.clone(), LinkTypes::AllImages, ())?;
    Ok(record)
}
#[hdk_extern]
pub fn get_file(original_file_hash: ActionHash) -> ExternResult<Option<Record>> {
    let links = get_links(GetLinksInputBuilder::try_new(
        original_file_hash.clone(),
        LinkTypes::FileUpdates,
    )?.build())?;
    let latest_link = links
        .into_iter()
        .max_by(|link_a, link_b| link_b.timestamp.cmp(&link_a.timestamp));
    let latest_file_hash = match latest_link {
        Some(link) => link.target.into_any_dht_hash().ok_or(wasm_error!(WasmErrorInner::Guest(String::from("Failed to convert link target to AnyDhtHash"))))?,
        None => AnyDhtHash::from(original_file_hash),
    };
    get(latest_file_hash, GetOptions::default())
}
#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateFileInput {
    pub original_file_hash: ActionHash,
    pub previous_file_hash: ActionHash,
    pub updated_file: File,
}
#[hdk_extern]
pub fn update_file(input: UpdateFileInput) -> ExternResult<Record> {
    let updated_file_hash = update_entry(
        input.previous_file_hash.clone(),
        &input.updated_file,
    )?;
    create_link(
        input.original_file_hash.clone(),
        updated_file_hash.clone(),
        LinkTypes::FileUpdates,
        (),
    )?;
    let record = get(updated_file_hash.clone(), GetOptions::default())?
        .ok_or(
            wasm_error!(
                WasmErrorInner::Guest(String::from("Could not find the newly updated File"))
            ),
        )?;
    Ok(record)
}
#[hdk_extern]
pub fn delete_file(original_file_hash: ActionHash) -> ExternResult<ActionHash> {
    delete_entry(original_file_hash)
}
