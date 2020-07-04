use crate::prelude::*;
use holochain_wasmer_guest::*;

/// the string "anchor" as utf8 bytes
pub const ANCHOR: [u8; 6] = [0x61, 0x6e, 0x63, 0x68, 0x6f, 0x72];

#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes, Clone)]
pub struct Anchor {
    pub anchor_type: String,
    pub anchor_text: Option<String>,
}

impl Pathable for Anchor {

}

#[test]
#[cfg(test)]
fn hash_path_anchor() {
    assert_eq!("anchor".as_bytes(), ANCHOR,);
}

#[cfg(test)]
#[test]
fn hash_path_classic_entry_def() {

    assert_eq!(
        EntryDefId::from("anchor"),
        Anchor::entry_def_id(),
    );

    assert_eq!(
        CrdtType,
        Anchor::crdt_type(),
    );

    assert_eq!(
        RequiredValidations::default(),
        Anchor::required_validations(),
    );

    assert_eq!(
        EntryVisibility::Public,
        Anchor::entry_visibility(),
    );

    assert_eq!(
        EntryDef {
            id: "anchor".into(),
            crdt_type: CrdtType,
            required_validations: RequiredValidations::default(),
            visibility: EntryVisibility::Public,
        },
        Anchor::entry_def(),
    );

}
