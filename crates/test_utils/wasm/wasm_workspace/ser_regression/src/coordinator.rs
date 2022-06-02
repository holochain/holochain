use crate::integrity::*;
use hdk::prelude::*;

fn channels_path() -> Path {
    let path = Path::from("channels")
        .try_into_typed(LinkTypes::Path)
        .unwrap();
    path.ensure().expect("Couldn't ensure path");
    path.into()
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes)]
pub struct CreateMessageInput {
    pub channel_hash: EntryHash,
    pub content: String,
}

#[hdk_extern]
fn create_channel(name: String) -> ExternResult<EntryHash> {
    debug!("channel name {:?}", name);
    let path = channels_path();
    let channel = Channel::new(name);
    let channel_hash = hash_entry(&channel)?;
    let sb: SerializedBytes = channel_hash.clone().try_into().unwrap();
    create_entry(&EntryTypes::Channel(channel))?;
    debug!("sb in channel {:?}", sb);
    create_link(
        path.path_entry_hash()?,
        channel_hash.clone(),
        LinkTypes::Any,
        (),
    )?;
    Ok(channel_hash)
}

#[hdk_extern]
fn create_message(input: CreateMessageInput) -> ExternResult<EntryHash> {
    debug!("{:?}", input);
    let CreateMessageInput {
        channel_hash,
        content,
    } = input;
    let message = ChannelMessage::new(content);
    let message_hash = hash_entry(&message)?;
    create_entry(&EntryTypes::ChannelMessage(message))?;
    create_link(channel_hash, message_hash.clone(), LinkTypes::Any, ())?;
    Ok(message_hash)
}
