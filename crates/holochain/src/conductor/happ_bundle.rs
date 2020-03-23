//! A hApp Bundle is a small bit of configuration which declares a collection of DNA
//! to be used by some UI

use derive_more::{From, Into};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use sx_types::{cell::CellId, prelude::HashString, shims::AgentPubKey};
use url::Url;

/// An ID used to refer to DNA in a Bundle
#[derive(Deserialize, Serialize, From, Into)]
pub struct DnaId(String);

/// An ID used to refer to a bundle elsewhere in the `ConductorConfig`
///
/// This name does not have to universally unique wrt. all other Bundles in the world,
/// but it *does* have to be unique wrt. the other Bundles within a single Conductor.
#[derive(Deserialize, Serialize, From, Into)]
pub struct BundleId(String);

/// A friendly name used to refer to a bundle. For display purposes only.
#[derive(Deserialize, Serialize, From, Into)]
pub struct BundleName(String);

/// The hApp bundle data format.
///
/// Describes which DNAs will be grouped together under what kind of [Interface].
pub struct Bundle {

    /// The default ID which this Bundle will be known by.
    /// This ID can be overridden by an Admin method, in case of a namespace collision with another Bundle.
    id: BundleId,

    /// Friendly name for the bundle, usually containing a hApp's name.
    name: BundleName,

    /// The DNAs that must be present on the conductor for this hApp to run.
    dnas: Vec<BundledDna>,

    /// Description of the Interface that the Conductor must run for this hApp to run.
    ///
    /// The [ConductorConfig] specifies the actual parameters needed to run an actual Interface,
    /// e.g. this description may specify that a Websocket interface is needed, but does not specify
    /// the port.
    ///
    /// The actual mapping between running Interfaces and the bundled DNAs happens in another place (TODO: where?)
    interface: BundledInterface,
}

/// Describes a DNA file to be included in a hApp bundle
pub struct BundledDna {
    /// This ID is not referenced anywhere else.
    /// It is only used by the Admin API to assign Agents to DNAs in this bundle when installing the bundle.
    id: DnaId,

    /// Hash of the DNA
    hash: HashString,

    /// Method used to fetch the DNA locally or remotely
    locator: DnaLocator,
}

/// Describes a method of fetching a DNA file locally or remotely
pub enum DnaLocator {
    /// Find DNA on local filesystem (for development)
    File(PathBuf),
    /// Fetch DNA from internet URL
    Url(Url),
    /// Fetch DNA from HCHC by hash
    HCHC,
}

/// Specifies what kind of [Interface] this hApp needs the Conductor to run
pub struct BundledInterface {
    /// Can the Conductor Admin API be accessed over this interface?
    admin: bool,
    /// The type of Interface which this hApp requires to run.
    /// The UI needs of the hApp determines this, e.g. if the UI is a webpage,
    /// Websocket would be an appropriate interface kind.
    kind: InterfaceKind,
}

pub enum InterfaceKind {
    Websocket,
    DomainSocket,
}
