use holo_hash::EntryHash;
use holochain_serialized_bytes::prelude::*;
use holochain_types::Entry;

#[tokio::test(threaded_scheduler)]
async fn hash_regression() {
    #[derive(Debug, Clone, Serialize, Deserialize, SerializedBytes, Eq, PartialEq)]
    struct CreateMessageInput {
        channel_hash: EntryHash,
        content: String,
    }
    #[derive(Serialize, Deserialize, SerializedBytes, Eq, PartialEq)]
    pub struct Channel {
        name: String,
    }

    impl Channel {
        pub fn new(name: String) -> Self {
            Self { name }
        }
    }

    impl TryFrom<&Channel> for Entry {
        type Error = SerializedBytesError;
        fn try_from(t: &Channel) -> Result<Self, Self::Error> {
            Ok(Entry::App(t.try_into()?))
        }
    }

    let channel = Channel::new("hello world".into());
    let entry: Entry = (&channel).try_into().unwrap();
    let channel_hash = EntryHash::with_data(&entry).await;
    // Probably to do with going to raw unsafe bytes
    // Or twice serialize
    let x = CreateMessageInput {
        channel_hash,
        content: "Hello from alice :)".into(),
    };
    let sb = SerializedBytes::try_from(x.clone()).unwrap();
    println!("{:?}", sb.bytes());
    println!("{:?}", sb);

    CreateMessageInput::try_from(sb).unwrap();
    let sb = SerializedBytes::try_from(x.clone()).unwrap();
    let sb_double = SerializedBytes::try_from(sb).unwrap();
    let sb = SerializedBytes::try_from(sb_double.clone()).unwrap();
    let x2 = CreateMessageInput::try_from(sb).unwrap();
    assert_eq!(x, x2);

    let sb = SerializedBytes::try_from(x.clone()).unwrap();
    let b = sb.bytes().clone();
    let sb2 = SerializedBytes::try_from(UnsafeBytes::from(b)).unwrap();
    let x2 = CreateMessageInput::try_from(sb2).unwrap();
    assert_eq!(x, x2);
}
