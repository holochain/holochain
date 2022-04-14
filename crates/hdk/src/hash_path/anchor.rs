use crate::hash_path::path::Component;
use crate::hash_path::path::Path;
use crate::prelude::*;
use holochain_wasmer_guest::*;

/// This is the root of the [ `Path` ] tree.
///
/// Forms the entry point to all anchors so that agents can navigate down the tree from here.
pub const ROOT: &[u8; 2] = &[0x00, 0x00];

#[derive(PartialEq, SerializedBytes, serde::Serialize, serde::Deserialize, Debug, Clone)]
/// An anchor can only be 1 or 2 levels deep as "type" and "text".
///
/// The second level is optional and the Strings use the standard [ `TryInto` ] for path [ `Component` ] internally.
///
/// __Anchors are required to be included in an application's [ `entry_defs` ]__ callback and so implement all the standard methods.
/// Technically the [ `Anchor` ] entry definition is the [ `Path` ] definition.
///
/// e.g. `entry_defs![Anchor::entry_def()]`
///
/// The methods implemented on anchor follow the patterns that predate the Path module but `Path::from(&anchor)` is always possible to use the newer APIs.
pub struct Anchor {
    pub anchor_type: String,
    pub anchor_text: Option<String>,
}

// Provide all the default entry conventions for anchors.
entry_def!(Anchor PathEntry::entry_def());

/// Anchors are just a special case of path, so we can move from anchor to path losslessly.
/// We simply format the anchor structure into a string that works with the path string handling.
impl From<&Anchor> for Path {
    fn from(anchor: &Anchor) -> Self {
        let mut components = vec![
            Component::new(ROOT.to_vec()),
            Component::from(anchor.anchor_type.as_bytes().to_vec()),
        ];
        if let Some(text) = anchor.anchor_text.as_ref() {
            components.push(Component::from(text.as_bytes().to_vec()));
        }
        components.into()
    }
}

/// Paths are more general than anchors so a path could be represented that is not a valid anchor.
/// The obvious example would be a path of binary data that is not valid utf-8 strings or a path
/// that is more than 2 levels deep.
impl TryFrom<&Path> for Anchor {
    type Error = WasmError;
    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        let components: Vec<Component> = path.as_ref().to_owned();
        if components.len() == 2 || components.len() == 3 {
            if components[0] == Component::new(ROOT.to_vec()) {
                Ok(Anchor {
                    anchor_type: std::str::from_utf8(components[1].as_ref())
                        .map_err(|e| SerializedBytesError::Deserialize(e.to_string()))?
                        .to_string(),
                    anchor_text: {
                        match components.get(2) {
                            Some(component) => Some(
                                std::str::from_utf8(component.as_ref())
                                    .map_err(|e| SerializedBytesError::Deserialize(e.to_string()))?
                                    .to_string(),
                            ),
                            None => None,
                        }
                    },
                })
            } else {
                Err(WasmError::Serialize(SerializedBytesError::Deserialize(
                    format!(
                        "Bad anchor path root {:0?} should be {:1?}",
                        components[0].as_ref(),
                        ROOT,
                    ),
                )))
            }
        } else {
            Err(WasmError::Serialize(SerializedBytesError::Deserialize(
                format!("Bad anchor path length {}", components.len()),
            )))
        }
    }
}

/// Simple string interface to simple string based paths.
/// a.k.a "the anchor pattern" that predates paths by a few years.
pub fn anchor(anchor_type: String, anchor_text: String) -> ExternResult<holo_hash::EntryHash> {
    let path: Path = (&Anchor {
        anchor_type,
        anchor_text: Some(anchor_text),
    })
        .into();
    path.ensure()?;
    path.path_entry_hash()
}

/// Returns every entry hash in a vector from the root of an anchor.
/// Hashes are sorted in the same way that paths sort children.
pub fn list_anchor_type_addresses() -> ExternResult<Vec<AnyLinkableHash>> {
    let links = Path::from(vec![Component::new(ROOT.to_vec())])
        .children()?
        .into_iter()
        .map(|link| link.target)
        .collect();
    Ok(links)
}

/// Returns every entry hash in a vector from the second level of an anchor.
/// Uses the string argument to build the path from the root.
/// Hashes are sorted in the same way that paths sort children.
pub fn list_anchor_addresses(anchor_type: String) -> ExternResult<Vec<AnyLinkableHash>> {
    let path: Path = (&Anchor {
        anchor_type,
        anchor_text: None,
    })
        .into();
    let links = path
        .children()?
        .into_iter()
        .map(|link| link.target)
        .collect();
    Ok(links)
}

/// Old version of holochain that anchors was designed for had two part link tags but now link
/// tags are a single array of bytes, so to get an external interface that is somewhat backwards
/// compatible we need to rebuild the anchors from the paths serialized into the links and then
/// return them.
pub fn list_anchor_tags(anchor_type: String) -> ExternResult<Vec<String>> {
    let path: Path = (&Anchor {
        anchor_type,
        anchor_text: None,
    })
        .into();
    path.ensure()?;
    let hopefully_anchor_tags: Result<Vec<String>, WasmError> = path
        .children_paths()?
        .into_iter()
        .map(|path| match Anchor::try_from(&path) {
            Ok(anchor) => match anchor.anchor_text {
                Some(text) => Ok(text),
                None => Err(WasmError::Serialize(SerializedBytesError::Deserialize(
                    "missing anchor text".into(),
                ))),
            },
            Err(e) => Err(e),
        })
        .collect();
    let mut anchor_tags = hopefully_anchor_tags?;
    anchor_tags.sort();
    anchor_tags.dedup();
    Ok(anchor_tags)
}

#[cfg(test)]
#[test]
fn hash_path_root() {
    assert_eq!(ROOT, &[0_u8, 0]);
}

#[cfg(test)]
#[test]
fn hash_path_anchor_path() {
    let examples = [
        (
            "foo",
            None,
            Path::from(vec![
                Component::from(vec![0, 0]),
                Component::from(vec![102, 111, 111]),
            ]),
        ),
        (
            "foo",
            Some("bar".to_string()),
            Path::from(vec![
                Component::from(vec![0, 0]),
                Component::from(vec![102, 111, 111]),
                Component::from(vec![98, 97, 114]),
            ]),
        ),
    ];
    for (atype, text, path) in examples {
        assert_eq!(
            path,
            (&Anchor {
                anchor_type: atype.to_string(),
                anchor_text: text,
            })
                .into(),
        );
    }
}

// #[cfg(test)]
// #[test]
// fn hash_path_anchor_entry_def() {
//     assert_eq!(PathEntry::ENTRY_DEFS, Anchor::ENTRY_DEFS,);
// }

#[cfg(test)]
#[test]
fn hash_path_anchor_from_path() {
    let path = Path::from(vec![
        Component::from(vec![0, 0]),
        Component::from(vec![102, 111, 111]),
        Component::from(vec![98, 97, 114]),
    ]);

    assert_eq!(
        Anchor::try_from(&path).unwrap(),
        Anchor {
            anchor_type: "foo".into(),
            anchor_text: Some("bar".into()),
        },
    );
}
