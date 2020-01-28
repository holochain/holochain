use crate::{
    agent::AgentId,
    chain_header::ChainHeader,
    link::{Link, LinkActionKind},
};
use agent::test_agent_id;
use chain_header::test_chain_header;
use holochain_json_api::{error::JsonError, json::JsonString};
use holochain_persistence_api::cas::content::Address;
use link::example_link;

//-------------------------------------------------------------------------------------------------
// LinkData
//-------------------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, DefaultJson)]
pub struct LinkData {
    pub action_kind: LinkActionKind,
    pub link: Link,
    pub top_chain_header: ChainHeader,
    agent_id: AgentId,
}

impl LinkData {
    pub fn new_add(
        base: &Address,
        target: &Address,
        tag: &str,
        link_type: &str,
        top_chain_header: ChainHeader,
        agent_id: AgentId,
    ) -> Self {
        LinkData {
            action_kind: LinkActionKind::ADD,
            link: Link::new(base, target, link_type, tag),
            top_chain_header,
            agent_id,
        }
    }

    pub fn new_delete(
        base: &Address,
        target: &Address,
        tag: &str,
        link_type: &str,
        top_chain_header: ChainHeader,
        agent_id: AgentId,
    ) -> Self {
        LinkData {
            action_kind: LinkActionKind::REMOVE,
            link: Link::new(base, target, link_type, tag),
            top_chain_header,
            agent_id,
        }
    }

    pub fn action_kind(&self) -> &LinkActionKind {
        &self.action_kind
    }

    pub fn link(&self) -> &Link {
        &self.link
    }

    pub fn from_link(
        link: &Link,
        action_kind: LinkActionKind,
        top_chain_header: ChainHeader,
        agent_id: AgentId,
    ) -> Self {
        LinkData {
            action_kind,
            link: link.clone(),
            top_chain_header,
            agent_id,
        }
    }

    pub fn add_from_link(link: &Link, top_chain_header: ChainHeader, agent_id: AgentId) -> Self {
        Self::from_link(link, LinkActionKind::ADD, top_chain_header, agent_id)
    }

    pub fn remove_from_link(link: &Link, top_chain_header: ChainHeader, agent_id: AgentId) -> Self {
        Self::from_link(link, LinkActionKind::REMOVE, top_chain_header, agent_id)
    }
}

pub fn example_link_add() -> LinkData {
    let link = example_link();
    LinkData::new_add(
        link.base(),
        link.target(),
        link.tag(),
        "foo-link-type",
        test_chain_header(),
        test_agent_id(),
    )
}

#[cfg(test)]
pub mod tests {

    use crate::{
        entry::{test_entry_a, test_entry_b, Entry},
        link::{
            example_link, example_link_action_kind, example_link_type, link_data::example_link_add,
        },
    };
    use holochain_json_api::json::JsonString;
    use holochain_persistence_api::cas::content::AddressableContent;
    use std::convert::TryFrom;

    pub fn test_link_entry() -> Entry {
        Entry::LinkAdd(example_link_add())
    }

    pub fn test_link_entry_json_string() -> JsonString {
        JsonString::from_json(&format!(
            "{{\"LinkAdd\":{{\"action_kind\":\"ADD\",\"link\":{{\"base\":\"{}\",\"target\":\"{}\",\"link_type\":\"foo-link-type\",\"tag\":\"foo-link-tag\"}},\"top_chain_header\":{{\"entry_type\":{{\"App\":\"testEntryType\"}},\"entry_address\":\"Qma6RfzvZRL127UCEVEktPhQ7YSS1inxEFw7SjEsfMJcrq\",\"provenances\":[[\"HcScIkRaAaaaaaaaaaAaaaAAAAaaaaaaaaAaaaaAaaaaaaaaAaaAAAAatzu4aqa\",\"sig\"]],\"link\":null,\"link_same_type\":null,\"link_update_delete\":null,\"timestamp\":\"2018-10-11T03:23:38+00:00\"}},\"agent_id\":{{\"nick\":\"bob\",\"pub_sign_key\":\"HcScIkRaAaaaaaaaaaAaaaAAAAaaaaaaaaAaaaaAaaaaaaaaAaaAAAAatzu4aqa\"}}}}}}",
            test_entry_a().address(),
            test_entry_b().address(),
        ))
    }

    #[test]
    fn link_smoke_test() {
        example_link();
    }

    #[test]
    fn link_base_test() {
        assert_eq!(&test_entry_a().address(), example_link().base(),);
    }

    #[test]
    fn link_target_test() {
        assert_eq!(&test_entry_b().address(), example_link().target(),);
    }

    #[test]
    fn link_type_test() {
        assert_eq!(&example_link_type(), example_link().link_type(),);
    }

    #[test]
    fn link_entry_smoke_test() {
        test_link_entry();
    }

    #[test]
    fn link_add_action_kind_test() {
        assert_eq!(
            &example_link_action_kind(),
            example_link_add().action_kind(),
        );
    }

    #[test]
    fn link_add_link_test() {
        assert_eq!(&example_link(), example_link_add().link(),);
    }

    #[test]
    /// show ToString for LinkAdd
    fn link_entry_to_string_test() {
        assert_eq!(
            test_link_entry_json_string(),
            JsonString::from(test_link_entry()),
        );
    }

    #[test]
    /// show From<String> for LinkAdd
    fn link_entry_from_string_test() {
        assert_eq!(
            Entry::try_from(test_link_entry_json_string()).unwrap(),
            test_link_entry(),
        );
    }
}
