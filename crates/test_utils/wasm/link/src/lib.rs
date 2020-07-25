use hdk3::prelude::*;
use hdk3::prelude::link::Links;

entry_defs!(vec![Path::entry_def()]);

map_extern!(link_entries, _link_entries);
map_extern!(remove_link, _remove_link);
map_extern!(get_links, _get_links);

fn path(s: &str) -> Result<EntryHash, WasmError> {
    let path = Path::from(s);
    path.ensure()?;
    Ok(path.hash()?)
}

fn base() -> Result<EntryHash, WasmError> {
    path("a")
}

fn target() -> Result<EntryHash, WasmError> {
    path("b")
}

fn _link_entries(_: ()) -> Result<HeaderHash, WasmError> {
    debug!("_link_entries")?;
    Ok(link_entries!(base()?, target()?)?)
}

fn _remove_link(input: RemoveLinkInput) -> Result<HeaderHash, WasmError> {
    debug!("_remove_link")?;
    debug!(&input)?;
    Ok(remove_link!(input.into_inner())?)
}

fn _get_links(_: ()) -> Result<Links, WasmError> {
    debug!("_get_links")?;
    Ok(get_links!(base()?)?)
}
