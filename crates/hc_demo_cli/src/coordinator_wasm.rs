use hdk::prelude::*;

super::wasm_common!();

#[hdk_extern]
pub fn init(_: ()) -> ExternResult<InitCallbackResult> {
    Ok(InitCallbackResult::Pass)
}

#[hdk_extern]
pub fn create_file(file: File) -> ExternResult<Record> {
    let file_hash = create_entry(&EntryTypes::File(file))?;
    let record = get(file_hash.clone(), GetOptions::default())?.ok_or(wasm_error!(
        WasmErrorInner::Guest(String::from("Could not find the newly created File"))
    ))?;
    let path = Path::from("all_files");
    create_link(path.path_entry_hash()?, file_hash, LinkTypes::AllFiles, ())?;
    Ok(record)
}

#[hdk_extern]
pub fn get_file(hash: ActionHash) -> ExternResult<Option<File>> {
    let record: Record = match get(hash, GetOptions::default())? {
        Some(r) => r,
        None => return Ok(None),
    };

    record.entry.to_app_option().map_err(|e| wasm_error!(e))
}

#[hdk_extern]
pub fn get_all_files(_: ()) -> ExternResult<Vec<ActionHash>> {
    let path = Path::from("all_files");
    let links = get_links(
        GetLinksInputBuilder::try_new(path.path_entry_hash()?, LinkTypes::AllFiles)?.build(),
    )?;
    let get_input: Vec<GetInput> = links
        .into_iter()
        .map(|link| {
            GetInput::new(
                ActionHash::try_from(link.target).unwrap().into(),
                GetOptions::default(),
            )
        })
        .collect();
    let records = HDK.with(|hdk| hdk.borrow().get(get_input))?;
    let hashes: Vec<ActionHash> = records
        .into_iter()
        .flatten()
        .map(|r| r.action_address().clone())
        .collect();
    Ok(hashes)
}
