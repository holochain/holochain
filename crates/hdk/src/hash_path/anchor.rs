use crate::hash_path::path::Component;
use crate::hash_path::path::Path;
use crate::prelude::*;
use holochain_wasmer_guest::*;
use holochain_zome_types::link::LinkTag;

/// "hdk.path.anchor.root"
pub const ROOT: &str = "hdk.path.anchor.root";

#[derive(PartialEq, serde::Serialize, serde::Deserialize, Debug, SerializedBytes, Clone)]
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
            anchor.anchor_text.as_ref().unwrap_or(&String::default())
        ))
    }
}

impl TryFrom<&Path> for Anchor {
    type Error = SerializedBytesError;
    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        let components: Vec<Component> = path.as_ref().to_owned();
        if components.len() == 2 || components.len() == 3 {
            if components[0] == Component::from(ROOT) {
                Ok(Anchor {
                    anchor_type: (&components[1]).try_into()?,
                    anchor_text: {
                        match components.get(2) {
                            Some(component) => Some(component.try_into()?),
                            None => None,
                        }
                    },
                })
            } else {
                Err(SerializedBytesError::FromBytes(
                    "Bad anchor path root".into(),
                ))
            }
        } else {
            Err(SerializedBytesError::FromBytes(format!(
                "Bad anchor path length {}",
                components.len()
            )))
        }
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
}

/// simple string interface to simple string based paths
/// a.k.a "the anchor pattern" that predates paths by a few years
pub fn anchor(
    anchor_type: String,
    anchor_text: String,
) -> Result<holo_hash_core::HoloHashCore, WasmError> {
    let path: Path = (&Anchor {
        anchor_type,
        anchor_text: Some(anchor_text),
    })
        .into();
    path.touch()?;
    Ok(path.pwd()?)
}

pub fn get_anchor(anchor_address: HoloHashCore) -> Result<Option<Anchor>, WasmError> {
    Ok(match get_entry!(anchor_address)? {
        Some(Entry::App(sb)) => {
            let path = Path::try_from(sb)?;
            Some(Anchor::try_from(&path)?)
        }
        _ => None,
    })
}

pub fn list_anchor_type_addresses() -> Result<Vec<holo_hash_core::HoloHashCore>, WasmError> {
    let links = Path::from(ROOT)
        .ls()?
        .into_inner()
        .into_iter()
        .map(|link| link.target)
        .collect();
    Ok(links)
}

pub fn list_anchor_addresses(
    anchor_type: String,
) -> Result<Vec<holo_hash_core::HoloHashCore>, WasmError> {
    let path: Path = (&Anchor {
        anchor_type: anchor_type,
        anchor_text: None,
    })
        .into();
    path.touch()?;
    let links = path
        .ls()?
        .into_inner()
        .into_iter()
        .map(|link| link.target)
        .collect();
    Ok(links)
}

/// @TODO not sure if this is useful or done correctly??
/// the whole idea of link tags has been removed since the old anchors implementation and even the
/// old version only returned the same thing as the anchor text
/// in this version we just remove the same link tag no matter what because all anchor links have
/// the same tag
pub fn list_anchor_tags(anchor_type: String) -> Result<Vec<LinkTag>, WasmError> {
    let path: Path = (&Anchor {
        anchor_type: anchor_type,
        anchor_text: None,
    })
        .into();
    path.touch()?;
    let mut tags: Vec<LinkTag> = path
        .ls()?
        .into_inner()
        .into_iter()
        .map(|link| link.tag)
        .collect();
    tags.sort();
    tags.dedup();
    Ok(tags)
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
fn hash_path_anchor_entry_def() {
    assert_eq!(Path::entry_def_id(), Anchor::entry_def_id(),);

    assert_eq!(Path::crdt_type(), Anchor::crdt_type(),);

    assert_eq!(Path::required_validations(), Anchor::required_validations(),);

    assert_eq!(Path::entry_visibility(), Anchor::entry_visibility(),);

    assert_eq!(Path::entry_def(), Anchor::entry_def(),);
}

#[cfg(test)]
#[test]
fn hash_path_anchor_from_path() {
    let path = Path::from(vec![
        Component::from(vec![
            104, 0, 0, 0, 100, 0, 0, 0, 107, 0, 0, 0, 46, 0, 0, 0, 112, 0, 0, 0, 97, 0, 0, 0, 116,
            0, 0, 0, 104, 0, 0, 0, 46, 0, 0, 0, 97, 0, 0, 0, 110, 0, 0, 0, 99, 0, 0, 0, 104, 0, 0,
            0, 111, 0, 0, 0, 114, 0, 0, 0, 46, 0, 0, 0, 114, 0, 0, 0, 111, 0, 0, 0, 111, 0, 0, 0,
            116, 0, 0, 0,
        ]),
        Component::from(vec![102, 0, 0, 0, 111, 0, 0, 0, 111, 0, 0, 0]),
        Component::from(vec![98, 0, 0, 0, 97, 0, 0, 0, 114, 0, 0, 0]),
    ]);

    assert_eq!(
        Anchor::try_from(&path).unwrap(),
        Anchor {
            anchor_type: "foo".into(),
            anchor_text: Some("bar".into()),
        },
    );
}
