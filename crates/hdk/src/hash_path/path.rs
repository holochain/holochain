use crate::prelude::*;
use hdi::hash_path::path::{root_hash, Component, TypedPath};

pub trait HdkPathExt {
    fn children(&self) -> ExternResult<Vec<holochain_zome_types::link::Link>>;
    fn children_paths(&self) -> ExternResult<Vec<TypedPath>>;
    fn children_details(&self) -> ExternResult<holochain_zome_types::link::LinkDetails>;
    fn ensure(&self) -> ExternResult<()>;
    fn exists(&self) -> ExternResult<bool>;
}

impl HdkPathExt for TypedPath {
    /// Touch and list all the links from this path to paths below it.
    /// Only returns links between paths, not to other entries that might have their own links.
    fn children(&self) -> ExternResult<Vec<holochain_zome_types::link::Link>> {
        Self::ensure(self)?;
        let mut unwrapped = get_links(
            GetLinksInputBuilder::try_new(
                self.path_entry_hash()?,
                LinkTypeFilter::single_type(self.link_type.zome_index, self.link_type.zome_type),
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
    fn children_paths(&self) -> ExternResult<Vec<TypedPath>> {
        let children = self.children()?;
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
                let mut new_path = self.path.clone();
                if let Some(component) = maybe_component {
                    new_path.append_component(component);
                }
                new_path.into_typed(self.link_type)
            })
            .collect())
    }

    fn children_details(&self) -> ExternResult<holochain_zome_types::link::LinkDetails> {
        Self::ensure(self)?;
        get_link_details(
            self.path_entry_hash()?,
            LinkTypeFilter::single_type(self.link_type.zome_index, self.link_type.zome_type),
            Some(holochain_zome_types::link::LinkTag::new([])),
            GetOptions::default(),
        )
    }

    /// Recursively touch this and every parent that doesn't exist yet.
    fn ensure(&self) -> ExternResult<()> {
        if !self.exists()? {
            if self.is_root() {
                create_link(
                    root_hash()?,
                    self.path_entry_hash()?,
                    self.link_type,
                    self.make_tag()?,
                )?;
            } else if let Some(parent) = self.parent() {
                parent.ensure()?;
                create_link(
                    parent.path_entry_hash()?,
                    self.path_entry_hash()?,
                    self.link_type,
                    self.make_tag()?,
                )?;
            }
        }
        Ok(())
    }

    /// Does data exist at the hash we expect?
    fn exists(&self) -> ExternResult<bool> {
        if self.0.is_empty() {
            Ok(false)
        } else if self.is_root() {
            let this_paths_hash: AnyLinkableHash = self.path_entry_hash()?.into();
            let exists = get_links(
                GetLinksInputBuilder::try_new(
                    root_hash()?,
                    LinkTypeFilter::single_type(
                        self.link_type.zome_index,
                        self.link_type.zome_type,
                    ),
                )?
                .tag_prefix(self.make_tag()?)
                .build(),
            )?
            .iter()
            .any(|Link { target, .. }| *target == this_paths_hash);
            Ok(exists)
        } else {
            let parent = self
                .parent()
                .expect("Must have parent if not empty or root");
            let this_paths_hash: AnyLinkableHash = self.path_entry_hash()?.into();
            let exists = get_links(
                GetLinksInputBuilder::try_new(
                    parent.path_entry_hash()?,
                    LinkTypeFilter::single_type(
                        self.link_type.zome_index,
                        self.link_type.zome_type,
                    ),
                )?
                .tag_prefix(self.make_tag()?)
                .build(),
            )?
            .iter()
            .any(|Link { target, .. }| *target == this_paths_hash);
            Ok(exists)
        }
    }
}
