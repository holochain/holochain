use crate::hash_path::path::Path;
use crate::prelude::*;
use holochain_wasmer_guest::*;

/// "hdk.path.anchor.root"
pub const ROOT: &str = "hdk.path.anchor.root";

#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes, Clone)]
pub struct Anchor {
    pub anchor_type: String,
    pub anchor_text: Option<String>,
}

impl From<&Anchor> for Path {
    fn from(anchor: &Anchor) -> Self {
        Self::from(&format!(
            "{}/{}/{}",
            ROOT,
            anchor.anchor_type,
            (&anchor).anchor_text.as_ref().unwrap_or(&String::default())
        ))
    }
}

impl Anchor {
    pub fn entry_def_id() -> EntryDefId {
        Path::entry_def_id()
    }

    pub fn crdt_type() -> CrdtType {
        Path::crdt_type()
    }

    pub fn required_validations() -> RequiredValidations {
        Path::required_validations()
    }

    pub fn entry_visibility() -> EntryVisibility {
        Path::entry_visibility()
    }

    pub fn entry_def() -> EntryDef {
        Path::entry_def()
    }

    pub fn pwd(&self) -> Result<holo_hash_core::HoloHashCore, WasmError> {
        Path::from(self).pwd()
    }

    pub fn exists(&self) -> Result<bool, WasmError> {
        Path::from(self).exists()
    }

    pub fn touch(&self) -> Result<(), WasmError> {
        Path::from(self).touch()
    }

    pub fn ls(&self) -> Result<Vec<holochain_zome_types::link::Link>, WasmError> {
        Path::from(self).ls()
    }
}

pub fn anchor(
    anchor_type: String,
    anchor_text: String,
) -> Result<holo_hash_core::HoloHashCore, WasmError> {
    let anchor = Anchor {
        anchor_type,
        anchor_text: Some(anchor_text),
    };
    anchor.touch()?;
    Ok(anchor.pwd()?)
}

#[cfg(test)]
#[test]
fn hash_path_root() {
    assert_eq!(ROOT, "hdk.path.anchor.root");
}

#[cfg(test)]
#[test]
fn hash_path_anchor_path() {
    for (atype, text, path_string) in vec![
        ("foo", None, "hdk.path.anchor.root/foo"),
        (
            "foo",
            Some("bar".to_string()),
            "hdk.path.anchor.root/foo/bar",
        ),
    ] {
        assert_eq!(
            Path::from(path_string),
            (&Anchor {
                anchor_type: atype.to_string(),
                anchor_text: text,
            })
                .into(),
        );
    }
}

#[cfg(test)]
#[test]
fn hash_path_classic_entry_def() {
    assert_eq!(Path::entry_def_id(), Anchor::entry_def_id(),);

    assert_eq!(Path::crdt_type(), Anchor::crdt_type(),);

    assert_eq!(Path::required_validations(), Anchor::required_validations(),);

    assert_eq!(Path::entry_visibility(), Anchor::entry_visibility(),);

    assert_eq!(Path::entry_def(), Anchor::entry_def(),);
}
