//! Interfaces to manage Holochain applications (hApps) and call their functions.
//!
//! The Conductor is the central component of Holochain. It exposes WebSockets for clients to
//! connect to, processes incoming requests and orchestrates data flow and persistence.
//!
//! Refer to [Holochain's architecture](https://developer.holochain.org/concepts/2_application_architecture)
//! for more info. Read about hApp development in the
//! [Happ Development Kit (HDK) documentation](https://docs.rs/hdk/latest/hdk).
//!
//! There is a [Holochain client for JavaScript](https://github.com/holochain/holochain-client-js)
//! and a [Rust client](https://github.com/holochain/holochain-client-rust)
//! to connect to the Conductor.
//!
//! The Conductor API is split into Admin and App requests and responses. Each has
//! an associated enum [`AdminRequest`] and [`AppRequest`] that define and
//! document available calls.
//!
//! The admin interface generally manages the conductor itself, such as installing apps,
//! listing dnas, cells and apps, accessing state and metric information dumps and managing
//! agents.
//!
//! The app interface is smaller and focussed on interfacing with an app directly.
//! Notably the app interface allows calling functions exposed by the hApps'
//! modules, called DNAs. To discover a particular hApp's structure, its app
//! info can be requested.
//!
//! Additional information can be found in the [docs module][`crate::docs`].
//!

#[cfg(doc)]
pub mod docs;

mod admin_interface;
mod app_interface;
pub mod config;
pub mod peer_meta;
pub mod signal_subscription;
pub mod state;
pub mod state_dump;
pub mod storage_info;

pub use admin_interface::*;
pub use app_interface::*;
pub use config::*;
pub use peer_meta::*;
pub use state_dump::*;
pub use storage_info::*;

/// Exports the complete TypeScript binding tree for the conductor API.
///
/// Single entry point behind `make ts-bindings`. Exports the wire roots
/// ([`AdminRequest`], [`AdminResponse`], [`AppRequest`], [`AppResponse`],
/// `holochain_types::signal::Signal`),
/// [`JsonDump`] (carried as a JSON string by [`AdminResponse::StateDumped`],
/// not part of the msgpack body), and every upstream crate's `ts_alias!`
/// markers and hand-written impls — all from one process, since ts-rs only
/// merges declarations sharing an output file within a single process.
#[cfg(feature = "ts_rs")]
pub fn export_ts_bindings(cfg: &ts_rs::Config) -> Result<(), ts_rs::ExportError> {
    use ts_rs::TS;

    holochain_types::export_ts_bindings(cfg)?;
    AppAuthenticationTokenTs::export_all(cfg)?;
    NetworkMetricsMapTs::export_all(cfg)?;
    PeerMetaInfoMapTs::export_all(cfg)?;
    AdminRequest::export_all(cfg)?;
    AdminResponse::export_all(cfg)?;
    AppRequest::export_all(cfg)?;
    AppResponse::export_all(cfg)?;
    AppAuthenticationRequest::export_all(cfg)?;
    JsonDump::export_all(cfg)?;
    inject_public_release_tags(cfg.out_dir())?;
    Ok(())
}

/// Adds an api-extractor `@public` release tag to every generated declaration.
///
/// holochain-client-js's docs build (api-extractor) errors on an exported
/// declaration with no release tag, so every `.ts` file under `dir` is
/// rewritten with [`tag_exports_public`] first.
#[cfg(feature = "ts_rs")]
fn inject_public_release_tags(dir: &std::path::Path) -> Result<(), ts_rs::ExportError> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            inject_public_release_tags(&path)?;
        } else if path.extension().is_some_and(|ext| ext == "ts") {
            let source = std::fs::read_to_string(&path)?;
            std::fs::write(&path, tag_exports_public(&source))?;
        }
    }
    Ok(())
}

/// Returns `source` with a TSDoc `@public` tag on every exported declaration.
///
/// Appends ` * @public` to an existing TSDoc block, or adds a minimal `/**
/// @public */` block if there is none. Idempotent — a block already tagged
/// is left untouched. Only top-level `export` lines are considered.
#[cfg(feature = "ts_rs")]
fn tag_exports_public(source: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for line in source.lines() {
        if line.starts_with("export ") {
            if out.last().is_some_and(|prev| prev.trim() == "*/") {
                let block_start = out
                    .iter()
                    .rposition(|prev| prev.trim_start().starts_with("/**"))
                    .unwrap_or(out.len() - 1);
                if !out[block_start..]
                    .iter()
                    .any(|prev| prev.contains("@public"))
                {
                    out.insert(out.len() - 1, " * @public");
                }
            } else {
                out.extend(["/**", " * @public", " */"]);
            }
        }
        out.push(line);
    }
    let mut tagged = out.join("\n");
    if source.ends_with('\n') {
        tagged.push('\n');
    }
    tagged
}

#[cfg(all(test, feature = "ts_rs"))]
mod ts_export {
    #[test]
    fn tag_exports_public_appends_to_existing_block() {
        let source = "/**\n * A cell.\n */\nexport type CellId = string;\n";
        let expected = "/**\n * A cell.\n * @public\n */\nexport type CellId = string;\n";
        assert_eq!(super::tag_exports_public(source), expected);
    }

    #[test]
    fn tag_exports_public_creates_block_when_missing() {
        let source = "export type ActionHash = HoloHash;\n";
        let expected = "/**\n * @public\n */\nexport type ActionHash = HoloHash;\n";
        assert_eq!(super::tag_exports_public(source), expected);
    }

    #[test]
    fn tag_exports_public_is_idempotent() {
        let source = "/**\n * A cell.\n */\nexport type CellId = string;\n";
        let tagged = super::tag_exports_public(source);
        assert_eq!(super::tag_exports_public(&tagged), tagged);
    }

    #[test]
    fn tag_exports_public_ignores_field_docs() {
        let source = "export type Cell = { \n/**\n * The id.\n */\nid: string, };\n";
        let expected =
            "/**\n * @public\n */\nexport type Cell = { \n/**\n * The id.\n */\nid: string, };\n";
        assert_eq!(super::tag_exports_public(source), expected);
    }
}
