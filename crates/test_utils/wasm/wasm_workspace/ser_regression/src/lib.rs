use derive_more::*;
use hdk3::prelude::*;

#[derive(Debug, Serialize, Deserialize, SerializedBytes)]
struct CreateMessageInput {
    channel_hash: EntryHash,
    content: String,
}
#[derive(Debug, From, Into, Serialize, Deserialize, SerializedBytes)]
pub struct ChannelName(String);

#[derive(Constructor, Serialize, Deserialize, SerializedBytes)]
pub struct Channel {
    name: String,
}

#[derive(Constructor, Serialize, Deserialize, SerializedBytes)]
pub struct ChannelMessage {
    message: String,
}

const CHANNEL_ID: &str = "channel";

const CHANNEL_MESSAGE_ID: &str = "channel_message";

entry_def!(Channel EntryDef {
    id: CHANNEL_ID.into(),
    ..Default::default()
});

entry_def!(ChannelMessage EntryDef {
    id: CHANNEL_MESSAGE_ID.into(),
    ..Default::default()
});

entry_defs![
    Path::entry_def(),
    Channel::entry_def(),
    ChannelMessage::entry_def()
];

fn channels_path() -> Path {
    let path = Path::from("channels");
    path.ensure().expect("Couldn't ensure path");
    path
}

#[hdk(extern)]
fn create_channel(name: ChannelName) -> ExternResult<EntryHash> {
    debug!(format!("channel name {:?}", name))?;
    let path = channels_path();
    let channel = Channel::new(name.into());
    let channel_hash = entry_hash!(&channel)?;
    let sb: SerializedBytes = channel_hash.clone().try_into().unwrap();
    commit_entry!(&channel)?;
    debug!(format!("sb in channel {:?}", sb))?;
    link_entries!(entry_hash!(&path)?, channel_hash.clone())?;
    Ok(channel_hash)
}

#[hdk(extern)]
fn create_message(input: CreateMessageInput) -> ExternResult<EntryHash> {
    debug!(format!("{:?}", input))?;
    let CreateMessageInput {
        channel_hash,
        content,
    } = input;
    let message = ChannelMessage::new(content);
    let message_hash = entry_hash!(&message)?;
    commit_entry!(&message)?;
    link_entries!(channel_hash, message_hash.clone())?;
    Ok(message_hash)
}
