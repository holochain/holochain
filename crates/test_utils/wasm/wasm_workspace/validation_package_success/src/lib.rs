use hdk3::prelude::*;

#[hdk_entry(id = "song", required_validation_type = "custom")]
struct Song;

#[hdk_entry(id = "artist", required_validation_type = "custom")]
struct Artist;

entry_defs![Song::entry_def(), Artist::entry_def()];

#[hdk_extern]
fn validation_package(
    app_entry_type: AppEntryType,
) -> ExternResult<ValidationPackageCallbackResult> {
    let author: EntryHash = agent_info!()?.agent_initial_pubkey.into();

    let index = app_entry_type.id();
    match u8::from(index) {
        // Artist
        1 => {
            let links = get_links!(author)?.into_inner();
            let mut songs = Vec::with_capacity(30);
            // Need to post at least 30 songs to be an artist on this dht
            for link in links.into_iter().take(30) {
                match get!(link.target)? {
                    Some(song) => songs.push(song),
                    None => break,
                }
            }
            if songs.len() >= 30 {
                Ok(ValidationPackageCallbackResult::Success(
                    ValidationPackage::new(songs),
                ))
            } else {
                Ok(ValidationPackageCallbackResult::Fail(
                    "Need at least 30 songs to be an artist on this dht".to_string(),
                ))
            }
        }
        _ => Ok(ValidationPackageCallbackResult::Success(
            ValidationPackage::new(vec![]),
        )),
    }
}

#[hdk_extern]
fn commit_artist(_: ()) -> ExternResult<HeaderHash> {
    Ok(create_entry!(Artist)?)
}

#[hdk_extern]
fn commit_songs(_: ()) -> ExternResult<()> {
    let author: EntryHash = agent_info!()?.agent_initial_pubkey.into();
    let hash = hash_entry!(Song)?;
    for _ in 0..30 {
        create_entry!(Song)?;
        create_link!(author.clone(), hash.clone())?;
    }
    Ok(())
}
