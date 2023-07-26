use hdi::{prelude::ExternResult, hash_path::path::{path_try_into_typed, path_entry_hash}};
use holo_hash::AnyLinkableHash;
use holochain_serialized_bytes::SerializedBytesError;
use holochain_wasmer_guest::{WasmError, wasm_error, WasmErrorInner};
use holochain_zome_types::{ScopedLinkType, hash_path::{path::{Path, ROOT, Component}, anchor::Anchor}};

use super::path::{typed_path_ensure, typed_path_children_paths, typed_path_children};


/// Simple string interface to simple string based paths.
/// a.k.a "the anchor pattern" that predates paths by a few years.
pub fn anchor<T, E>(
  link_type: T,
  anchor_type: String,
  anchor_text: String,
) -> ExternResult<holo_hash::EntryHash>
where
  ScopedLinkType: TryFrom<T, Error = E>,
  WasmError: From<E>,
{
  let path: Path = (&Anchor {
      anchor_type,
      anchor_text: Some(anchor_text),
  })
      .into();
  let path = path_try_into_typed(path, link_type)?;
  typed_path_ensure(path)?;
  path_entry_hash(path.path)
}

/// Returns every hash in a vector from the root of an anchor.
/// Hashes are sorted in the same way that paths sort children.
pub fn list_anchor_type_addresses<T, E>(link_type: T) -> ExternResult<Vec<AnyLinkableHash>>
where
  ScopedLinkType: TryFrom<T, Error = E>,
  WasmError: From<E>,
{
  let links = typed_path_children(
        path_try_into_typed(
          Path::from(vec![Component::new(ROOT.to_vec())]), link_type
        )?
      )?
      .into_iter()
      .map(|link| link.target)
      .collect();
  Ok(links)
}

/// Returns every hash in a vector from the second level of an anchor.
/// Uses the string argument to build the path from the root.
/// Hashes are sorted in the same way that paths sort children.
pub fn list_anchor_addresses<T, E>(
  link_type: T,
  anchor_type: String,
) -> ExternResult<Vec<AnyLinkableHash>>
where
  ScopedLinkType: TryFrom<T, Error = E>,
  WasmError: From<E>,
{
  let path: Path = (&Anchor {
      anchor_type,
      anchor_text: None,
  })
      .into();
  let links = path
      .typed(link_type)?
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
pub fn list_anchor_tags<T, E>(link_type: T, anchor_type: String) -> ExternResult<Vec<String>>
where
  ScopedLinkType: TryFrom<T, Error = E>,
  WasmError: From<E>,
{
  let path: Path = (&Anchor {
      anchor_type,
      anchor_text: None,
  })
      .into();
  let path = path_try_into_typed(path, link_type)?;
  typed_path_ensure(path)?;
  let hopefully_anchor_tags: Result<Vec<String>, WasmError> = typed_path_children_paths(path)?
      .into_iter()
      .map(|path| match Anchor::try_from(&path.path) {
          Ok(anchor) => match anchor.anchor_text {
              Some(text) => Ok(text),
              None => Err(wasm_error!(WasmErrorInner::Serialize(
                  SerializedBytesError::Deserialize("missing anchor text".into(),)
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