use holochain_2020::core::state::cascade::Cascade;
use sx_types::persistence::cas::content::AddressableContent;
use sx_types::agent::AgentId;
use sx_types::entry::Entry;

#[tokio::test]
async fn get() {
    let jimbo = Entry::AgentId(AgentId::generate_fake("Jimbo"));
    let address = jimbo.address();
    let entry = Cascade::get(address).await;
    assert_eq!(entry, jimbo);
}

