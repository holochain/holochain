pub mod constants;
pub mod path;
pub mod shard;

use crate::anchor::constants::ANCHOR;
use crate::prelude::*;
use holochain_wasmer_guest::*;

#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes, Clone)]
pub struct Anchor {
    pub anchor_type: String,
    pub anchor_text: Option<String>,
}

impl Anchor {
    pub fn entry_def_id() -> EntryDefId {
        core::str::from_utf8(&ANCHOR).unwrap().into()
    }

    pub fn entry_def() -> EntryDef {
        EntryDef {
            id: Anchor::entry_def_id(),
            crdt_type: CrdtType,
            required_validations: RequiredValidations::default(),
            visibility: EntryVisibility::Public,
        }
    }
}

// // in hdk
//
// struct Anchor {
//     type: String,
//     text: Option<String>,
// }
//
// /// the number/depth of nested anchors that we use to avoid hotspots
// struct ShardFactor;
// impl Default for ShardFactor {
//     fn default() -> Self {
//         Self(0)
//     }
// }
// /// the start of the path e.g. users/...
// struct AnchorType;
// /// the end of the path e.g. .../thedavidmeister
// struct AnchorText;
//
// impl From<(AnchorType, AnchorText, ShardFactor)> for Anchor {
//  // ...
// }
//
// impl From<(AnchorType, AnchorText)> for Anchor {
//   fn from(t: (AnchorType, AnchorText)) -> Self {
//       let (type, text) = t;
//       (ShardFactor::default(), type, text).into()
//   }
// }
//
// // in wasm
// struct User(String)
// impl From<&User> for ShardFactor {
//     fn from(user: &User) -> Self {
//         Self::from(3)
//     }
// }
// impl From<&User> for AnchorType {
//     fn from(user: &User) -> Self {
//         Self::from("users")
//     }
// }
// impl From<&User> for AnchorText {
//     fn from(user: &User) -> Self {
//         Self::from(user.0)
//     }
// }
//
// let user = User::from("thedavidmeister");
// Anchor::from((ShardFactor::from(&user), AnchorType::from(&user), AnchorText::from(&user)));
// // "root/users/t/h/e/thedavidmeister" -> vec!["root".as_bytes(), "users".as_bytes(), "t".as_bytes() , ...]

// impl TryFrom<Anchor> for String {
//     type Error = core::str::Utf8Error;
//     fn try_from(anchor: Anchor) -> Result<Self, Self::Error> {
//         let string_components: Result<Vec<&str>, core::str::Utf8Error> = anchor.0.iter().map(|c| core::str::from_utf8(&c.0)).collect();
//         Ok(string_components?.join(DELIMITER).to_string())
//     }
// }
//
// impl TryFrom<&Anchor> for Entry {
//     type Error = SerializedBytesError;
//     fn try_from(anchor: &Anchor) -> Result<Self, Self::Error> {
//         Ok(Self::App(anchor.try_into()?))
//     }
// }
//
// impl TryFrom<&Anchor> for EntryHashInput {
//     type Error = SerializedBytesError;
//     fn try_from(anchor: &Anchor) -> Result<Self, Self::Error> {
//         Ok(Self(anchor.try_into()?))
//     }
// }
//
// impl Anchor {
//     pub fn entry_def_id() -> EntryDefId {
//         core::str::from_utf8(&ANCHOR).unwrap().into()
//     }
//
//     pub fn entry_def() -> EntryDef {
//         EntryDef {
//             id: Anchor::entry_def_id(),
//             crdt_type: CrdtType,
//             required_validations: RequiredValidations::default(),
//             visibility: EntryVisibility::Public,
//         }
//     }
//
//     /// does an entry exist at the hash we expect?
//     /// something like `[ -d $DIR ]`
//     pub fn exists(&self) -> Result<bool, WasmError> {
//         get_entry!(entry_hash!(self)?)?.is_some()
//     }
//
//     /// recursively touch this and every parent that doesn't exist yet
//     /// something like `mkdir -p $DIR`
//     pub fn touch(&self) -> Result<(), WasmError> {
//         if ! self.exists() {
//             commit_entry!(self)?;
//             let parent = Anchor::from(self.as_ref()[0..self.as_ref().len()-1].to_vec()).touch()?;
//             link_entries!(parent, self, holochain_zome_types::link::LinkTag::from(ANCHOR))?;
//         }
//     }
//
//     /// touch and list all the links from this anchor to anchors below it
//     /// only returns links between anchors, not to other entries that might have their own links
//     /// something like `mkdir -p $DIR && ls -d $DIR`
//     pub fn ls(&self) -> Result<Vec<holochain_zome_types::link::Link>, WasmError> {
//         Anchor::touch(&self)?;
//         get_links!(&self, holochain_zome_types::link::LinkTag::from(ANCHOR))?;
//     }
// }
