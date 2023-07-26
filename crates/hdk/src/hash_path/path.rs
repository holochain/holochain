
use hdi::hash_path::path::path_entry_hash;
use hdi::hash_path::path::path_into_typed;
use hdi::hash_path::path::path_make_tag;
use hdi::hash_path::path::root_hash;
use hdi::prelude::ExternResult;
use holo_hash::AnyLinkableHash;
use holochain_wasmer_guest::WasmError;
use holochain_wasmer_guest::wasm_error;
use holochain_zome_types::Link;
use holochain_zome_types::LinkTypeFilter;
use holochain_zome_types::hash_path::path::Component;
use holochain_zome_types::hash_path::path::Path;
use holochain_zome_types::hash_path::path::TypedPath;
use crate::link::GetLinksInputBuilder;
use crate::link::create_link;
use crate::link::get_link_details;
use crate::link::get_links;
use holochain_serialized_bytes::prelude::*;

/// Does data exist at the hash we expect?
pub fn typed_path_exists(tp: TypedPath) -> ExternResult<bool> {
    if tp.0.is_empty() {
        Ok(false)
    } else if tp.is_root() {
        let this_paths_hash: AnyLinkableHash = path_entry_hash(tp.path)?.into();
        let exists = get_links(
            GetLinksInputBuilder::try_new(
                root_hash()?,
                LinkTypeFilter::single_type(
                    tp.link_type.zome_index,
                    tp.link_type.zome_type,
                ),
            )?
            .tag_prefix(path_make_tag(tp.path)?)
            .build(),
        )?
        .iter()
        .any(|Link { target, .. }| *target == this_paths_hash);
        Ok(exists)
    } else {
        let parent = typed_path_parent(tp)
            .expect("Must have parent if not empty or root");
        let this_paths_hash: AnyLinkableHash = path_entry_hash(tp.path)?.into();
        let exists = get_links(
            GetLinksInputBuilder::try_new(
                path_entry_hash(parent.path)?,
                LinkTypeFilter::single_type(
                    tp.link_type.zome_index,
                    tp.link_type.zome_type,
                ),
            )?
            .tag_prefix(path_make_tag(tp.path)?)
            .build(),
        )?
        .iter()
        .any(|Link { target, .. }| *target == this_paths_hash);
        Ok(exists)
    }
}

/// Recursively touch this and every parent that doesn't exist yet.
pub fn typed_path_ensure(tp: TypedPath) -> ExternResult<()> {
    if !typed_path_exists(tp)? {
        if tp.is_root() {
            create_link(
                root_hash()?,
                path_entry_hash(tp.path)?,
                tp.link_type,
                path_make_tag(tp.path)?,
            )?;
        } else if let Some(parent) = typed_path_parent(tp) {
            typed_path_ensure(parent)?;
            create_link(
                path_entry_hash(parent.path)?,
                path_entry_hash(tp.path)?,
                tp.link_type,
                path_make_tag(tp.path)?,
            )?;
        }
    }
    Ok(())
}

  /// Touch and list all the links from this path to paths below it.
  /// Only returns links between paths, not to other entries that might have their own links.
  pub fn typed_path_children(tp: TypedPath) -> ExternResult<Vec<holochain_zome_types::link::Link>> {
    typed_path_ensure(tp)?;
    let mut unwrapped = get_links(
        GetLinksInputBuilder::try_new(
            path_entry_hash(tp.path)?,
            LinkTypeFilter::single_type(tp.link_type.zome_index, tp.link_type.zome_type),
        )?
        .build(),
    )?;
    // Only need one of each hash to build the tree.
    unwrapped.sort_unstable_by(|a, b| a.tag.cmp(&b.tag));
    unwrapped.dedup_by(|a, b| a.tag.eq(&b.tag));
    Ok(unwrapped)
}

/// Touch and list all the links from this path to paths below it.
/// Same as `Path::children` but returns `Vec<Path>` rather than `Vec<Link>`.
/// This is more than just a convenience. In general it's not possible to
/// construct a full `Path` from a child `Link` alone as only a single
/// `Component` is encoded into the link tag. To build a full child path
/// the parent path + child link must be combined, which this function does
/// to produce each child, by using `&self` as that parent.
pub fn typed_path_children_paths(tp: TypedPath) -> ExternResult<Vec<TypedPath>> {
    let children = typed_path_children(tp)?;
    let components: ExternResult<Vec<Option<Component>>> = children
        .into_iter()
        .map(|link| {
            let component_bytes = &link.tag.0[..];
            if component_bytes.is_empty() {
                Ok(None)
            } else {
                Ok(Some(
                    SerializedBytes::from(UnsafeBytes::from(component_bytes.to_vec()))
                        .try_into()
                        .map_err(|e: SerializedBytesError| wasm_error!(e))?,
                ))
            }
        })
        .collect();
    Ok(components?
        .into_iter()
        .map(|maybe_component| {
            let mut new_path = tp.path.clone();
            if let Some(component) = maybe_component {
                new_path.append_component(component);
            }
            path_into_typed(new_path, tp.link_type)
        })
        .collect())
}

pub fn typed_path_children_details(tp: TypedPath) -> ExternResult<holochain_zome_types::link::LinkDetails> {
    typed_path_ensure(tp)?;
    get_link_details(
        path_entry_hash(tp.path)?,
        LinkTypeFilter::single_type(tp.link_type.zome_index, tp.link_type.zome_type),
        Some(holochain_zome_types::link::LinkTag::new([])),
    )
}

/// The parent of the current path is simply the path truncated one level.
pub fn typed_path_parent(tp: TypedPath) -> Option<TypedPath> {
    if tp.path.as_ref().len() > 1 {
        let parent_vec: Vec<Component> =
            tp.path.as_ref()[0..tp.path.as_ref().len() - 1].to_vec();
        Some(path_into_typed(Path::from(parent_vec), tp.link_type))
    } else {
        None
    }
}
