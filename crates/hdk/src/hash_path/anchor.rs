use crate::hash_path::path::Component;
use crate::hash_path::path::Path;
use crate::prelude::*;
use holochain_wasmer_guest::*;

/// This is the root of the [ `Path` ] tree.
///
/// Forms the entry point to all anchors so that agents can navigate down the tree from here.
///
/// The string "hdkanchor".
pub const ROOT: &str = "hdkanchor";

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
entry_def!(Anchor Path::entry_def());

/// Anchors are just a special case of path, so we can move from anchor to path losslessly.
/// We simply format the anchor structure into a string that works with the path string handling.
impl From<&Anchor> for Path {
    fn from(anchor: &Anchor) -> Self {
        Self::from(&format!(
            "{1}{0}{2}{0}{3}",
            crate::hash_path::path::DELIMITER,
            ROOT,
            anchor.anchor_type,
            anchor.anchor_text.as_ref().unwrap_or(&String::default())
        ))
    }
}

/// Paths are more general than anchors so a path could be represented that is not a valid anchor.
/// The obvious example would be a path of binary data that is not valid utf-8 strings or a path
/// that is more than 2 levels deep.
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
                Err(SerializedBytesError::Deserialize(format!(
                    "Bad anchor path root {:0?} should be {:1?}",
                    components[0].as_ref(),
                    ROOT.as_bytes(),
                )))
            }
        } else {
            Err(SerializedBytesError::Deserialize(format!(
                "Bad anchor path length {}",
                components.len()
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
    path.hash()
}

/// Attempt to get an anchor by its hash.
/// Returns None if the hash doesn't point to an anchor.
/// We can't do anything fancy like ensure the anchor if not exists because we only have a hash.
pub fn get_anchor(anchor_address: EntryHash) -> ExternResult<Option<Anchor>> {
    Ok(
        match crate::prelude::get(anchor_address, GetOptions::content())?.and_then(|el| el.into()) {
            Some(Entry::App(eb)) => {
                let path = Path::try_from(SerializedBytes::from(eb))?;
                Some(Anchor::try_from(&path)?)
            }
            _ => None,
        },
    )
}

/// Returns every entry hash in a vector from the root of an anchor.
/// Hashes are sorted in the same way that paths sort children.
pub fn list_anchor_type_addresses() -> ExternResult<Vec<EntryHash>> {
    let links = Path::from(ROOT)
        .children()?
        .into_inner()
        .into_iter()
        .map(|link| link.target)
        .collect();
    Ok(links)
}

/// Returns every entry hash in a vector from the second level of an anchor.
/// Uses the string argument to build the path from the root.
/// Hashes are sorted in the same way that paths sort children.
pub fn list_anchor_addresses(anchor_type: String) -> ExternResult<Vec<EntryHash>> {
    let path: Path = (&Anchor {
        anchor_type,
        anchor_text: None,
    })
        .into();
    path.ensure()?;
    let links = path
        .children()?
        .into_inner()
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
    let hopefully_anchor_tags: Result<Vec<String>, SerializedBytesError> = path
        .children()?
        .into_inner()
        .into_iter()
        .map(|link| match Path::try_from(&link.tag) {
            Ok(path) => match Anchor::try_from(&path) {
                Ok(anchor) => match anchor.anchor_text {
                    Some(text) => Ok(text),
                    None => Err(SerializedBytesError::Deserialize(
                        "missing anchor text".into(),
                    )),
                },
                Err(e) => Err(e),
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
    assert_eq!(ROOT, "hdkanchor");
}

#[cfg(test)]
#[test]
fn hash_path_anchor_path() {
    for (atype, text, path_string) in vec![
        ("foo", None, "hdkanchor.foo"),
        ("foo", Some("bar".to_string()), "hdkanchor.foo.bar"),
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
            104, 0, 0, 0, 100, 0, 0, 0, 107, 0, 0, 0, 97, 0, 0, 0, 110, 0, 0, 0, 99, 0, 0, 0, 104,
            0, 0, 0, 111, 0, 0, 0, 114, 0, 0, 0,
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
