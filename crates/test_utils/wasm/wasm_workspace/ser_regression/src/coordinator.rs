use crate::integrity::*;
use hdk::prelude::*;

#[hdk_entry_zomes]
enum EntryZomes {
    IntegritySerRegression(EntryTypes),
}

#[derive(ToZomeName)]
enum Zomes {
    IntegritySerRegression,
}

#[hdk_link_zomes]
enum LinkZomes {
    IntegritySerRegression(LinkTypes),
}

impl EntryZomes {
    fn channel(channel: Channel) -> Self {
        Self::IntegritySerRegression(EntryTypes::Channel(channel))
    }
    fn message(message: ChannelMessage) -> Self {
        Self::IntegritySerRegression(EntryTypes::ChannelMessage(message))
    }
}

impl LinkZomes {
    fn any() -> Self {
        Self::IntegritySerRegression(LinkTypes::Any)
    }
}

fn channels_path() -> Path {
    let path = Path::from("channels").locate(Zomes::IntegritySerRegression);
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
    create_entry(&EntryZomes::channel(channel))?;
    debug!("sb in channel {:?}", sb);
    create_link(
        path.path_entry_hash()?.into(),
        channel_hash.clone().into(),
        LinkZomes::any(),
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
    create_entry(&EntryZomes::message(message))?;
    create_link(
        channel_hash.into(),
        message_hash.clone().into(),
        LinkZomes::any(),
        (),
    )?;
    Ok(message_hash)
}
