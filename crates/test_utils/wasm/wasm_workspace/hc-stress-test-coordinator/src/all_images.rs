use hdk::prelude::*;
use files_integrity::*;
#[hdk_extern]
pub fn get_all_images(_: ()) -> ExternResult<Vec<ActionHash>> {
    let path = Path::from("all_images");
    let links = get_links(GetLinksInputBuilder::try_new(
        path.path_entry_hash()?,
        LinkTypes::AllImages,
    )?.build())?;
    let get_input: Vec<GetInput> = links
        .into_iter()
        .map(|link| {

            let target_hash = match link.target.into_any_dht_hash() {
                Some(hash) => hash,
                None => return None
            };

            Some(GetInput::new(
            target_hash,
            GetOptions::default(),
            ))
        }).filter_map(|l| l)
        .collect();
    let records = HDK.with(|hdk| hdk.borrow().get(get_input))?;
    let hashes: Vec<ActionHash> = records
        .into_iter()
        .filter_map(|r| r)
        .map(|r| r.action_address().clone())
        .collect();
    Ok(hashes)
}
