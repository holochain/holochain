use crate::map_extern::ExternResult;
use crate::prelude::hash_entry;
use holo_hash::AnyLinkableHash;
use holochain_integrity_types::AppEntryBytes;
use holochain_integrity_types::Entry;
use holochain_integrity_types::LinkTag;
use holochain_integrity_types::ScopedLinkType;
use holochain_integrity_types::hash_path::path::Path;
use holochain_integrity_types::hash_path::path::ROOT;
use holochain_integrity_types::hash_path::path::TypedPath;
use holochain_wasmer_guest::SerializedBytes;
use holochain_wasmer_guest::UnsafeBytes;
use holochain_wasmer_guest::WasmError;
use holochain_wasmer_guest::wasm_error;

pub fn root_hash() -> ExternResult<AnyLinkableHash> {
  hash_entry(Entry::App(
      AppEntryBytes::try_from(SerializedBytes::from(UnsafeBytes::from(ROOT.to_vec())))
          .expect("This cannot fail as it's under the max entry bytes"),
  ))
  .map(Into::into)
}

/// Make the [`LinkTag`] for this [`Path`].
pub fn path_make_tag(p: Path) -> ExternResult<LinkTag> {
  Ok(LinkTag::new(match p.leaf() {
      None => <Vec<u8>>::with_capacity(0),
      Some(component) => {
          UnsafeBytes::from(SerializedBytes::try_from(component).map_err(|e| wasm_error!(e))?)
              .into()
      }
  }))
}

/// What is the hash for the current [ `Path` ]?
pub fn path_entry_hash(p: Path) -> ExternResult<holo_hash::EntryHash> {
  hash_entry(Entry::App(AppEntryBytes(
      SerializedBytes::try_from(p).map_err(|e| wasm_error!(e))?,
  )))
}

/// Attach a [`LinkType`] to this path
/// so its type is known for [`create_link`] and [`get_links`].
pub fn path_into_typed(p: Path, link_type: impl Into<ScopedLinkType>) -> TypedPath {
  TypedPath::new(link_type, p)
}

/// Try attaching a [`LinkType`] to this path
/// so its type is known for [`create_link`] and [`get_links`].
pub fn path_try_into_typed<TY, E>(p: Path, link_type: TY) -> Result<TypedPath, WasmError>
where
  ScopedLinkType: TryFrom<TY, Error = E>,
  WasmError: From<E>,
{
  Ok(TypedPath::new(ScopedLinkType::try_from(link_type)?, p))
}