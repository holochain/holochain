use holo_hash::*;
use holochain_types::prelude::*;
use holochain_zome_types::cell::CellId;
use kitsune_p2p::agent_store::AgentInfoSigned;

use crate::InstalledAppInfo;

/// Represents the available conductor functions to call over an Admin interface
/// and will result in a corresponding [`AdminResponse`] message being sent back over the
/// interface connection.
/// Enum variants follow a general convention of `verb_noun` as opposed to
/// the `noun_verb` of `AdminResponse`.
///
/// Expects a serialized object with any contents of the enum on a key `data`
/// and the enum variant on a key `type`, e.g.
/// `{ type: 'enable_app', data: { installed_app_id: 'test_app' } }`
///
/// [`AdminResponse`]: enum.AdminResponse.html
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum AdminRequest {
    /// Set up and register one or more new Admin interfaces
    /// as specified by a list of configurations. See [`AdminInterfaceConfig`]
    /// for details on the configuration.
    ///
    /// Will be responded to with an [`AdminResponse::AdminInterfacesAdded`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminInterfaceConfig`]: ../config/struct.AdminInterfaceConfig.html
    /// [`AdminResponse::AdminInterfacesAdded`]: enum.AdminResponse.html#variant.AdminInterfacesAdded
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    AddAdminInterfaces(Vec<crate::config::AdminInterfaceConfig>),

    /// Register a DNA for later use in InstallApp
    /// Stores the given DNA into the holochain dnas database and returns the hash of the DNA
    /// Will be responded to with an [`AdminResponse::DnaRegistered`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`RegisterDnaPayload`]: ../../../holochain_types/app/struct.RegisterDnaPayload.html
    /// [`AdminResponse::DnaRegistered`]: enum.AdminResponse.html#variant.DnaRegistered
    RegisterDna(Box<RegisterDnaPayload>),

    /// "Clone" a DNA (in the biological sense), thus creating a new Cell.
    ///
    /// Using the provided, already-registered DNA, create a new DNA with a unique
    /// UID and the specified properties, create a new Cell from this cloned DNA,
    /// and add the Cell to the specified App.
    ///
    /// Will be responded to with an [`AdminResponse::DnaCloned`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`CreateCloneCellPayload`]: ../../../holochain_types/app/struct.CreateCloneCellPayload.html
    /// [`AdminResponse::DnaCloned`]: enum.AdminResponse.html#variant.DnaCloned
    CreateCloneCell(Box<CreateCloneCellPayload>),

    /// Install an app from a list of `Dna` paths.
    /// Triggers genesis to be run on all `Cell`s and to be stored.
    /// An `App` is intended for use by
    /// one and only one Agent and for that reason it takes an `AgentPubKey` and
    /// installs all the Dnas with that `AgentPubKey` forming new `Cell`s.
    /// See [`InstallAppPayload`] for full details on the configuration.
    ///
    /// Note that the new `App` will not be enabled automatically after installation
    /// and can be enabled by calling [`AdminRequest::EnableApp`].
    ///
    /// Will be responded to with an [`AdminResponse::AppInstalled`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`InstallAppPayload`]: ../../../holochain_types/app/struct.InstallAppPayload.html
    /// [`AdminRequest::EnableApp`]: enum.AdminRequest.html#variant.EnableApp
    /// [`AdminResponse::AppInstalled`]: enum.AdminResponse.html#variant.AppInstalled
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    InstallApp(Box<InstallAppPayload>),

    /// Install an app using an [`AppBundle`].
    ///
    /// Triggers genesis to be run on all `Cell`s and to be stored.
    /// An `App` is intended for use by
    /// one and only one Agent and for that reason it takes an `AgentPubKey` and
    /// installs all the Dnas with that `AgentPubKey` forming new `Cell`s.
    /// See [`InstallAppBundlePayload`] for full details on the configuration.
    ///
    /// Note that the new `App` will not be enabled automatically after installation
    /// and can be enabled by calling [`AdminRequest::EnableApp`].
    ///
    /// Will be responded to with an [`AdminResponse::AppInstalled`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`InstallAppBundlePayload`]: ../../../holochain_types/app/struct.InstallAppBundlePayload.html
    /// [`AdminRequest::EnableApp`]: enum.AdminRequest.html#variant.EnableApp
    /// [`AdminResponse::AppInstalled`]: enum.AdminResponse.html#variant.AppInstalled
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    InstallAppBundle(Box<InstallAppBundlePayload>),

    /// Uninstalls the `App` specified by argument `installed_app_id` from the conductor,
    /// meaning that the app will be removed from the list of installed apps, and any Cells
    /// which were referenced only by this app will be disabled and removed, clearing up
    /// any persisted data. 
    /// Cells which are still referenced by other installed apps will not be removed.
    ///
    /// Will be responded to with an [`AdminResponse::AppUninstalled`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminResponse::AppUninstalled`]: enum.AdminResponse.html#variant.AppUninstalled
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    UninstallApp {
        /// The InstalledAppId to uninstall
        installed_app_id: InstalledAppId,
    },

    /// List the hashes of all installed `Dna`s.
    /// Takes no arguments.
    ///
    /// Will be responded to with an [`AdminResponse::DnasListed`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminResponse::DnasListed`]: enum.AdminResponse.html#variant.DnasListed
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    ListDnas,

    /// Generate a new AgentPubKey.
    /// Takes no arguments.
    ///
    /// Will be responded to with an [`AdminResponse::AgentPubKeyGenerated`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminResponse::AgentPubKeyGenerated`]: enum.AdminResponse.html#variant.AgentPubKeyGenerated
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    GenerateAgentPubKey,

    /// List all the cell ids in the conductor.
    /// Takes no arguments.
    ///
    /// Will be responded to with an [`AdminResponse::CellIdsListed`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminResponse::CellIdsListed`]: enum.AdminResponse.html#variant.CellIdsListed
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    ListCellIds,

    /// List the ids of all the enabled Apps in the conductor.
    /// Takes no arguments.
    ///
    /// Will be responded to with an [`AdminResponse::ActiveAppsListed`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminResponse::ActiveAppsListed`]: enum.AdminResponse.html#variant.ActiveAppsListed
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    ListEnabledApps,

    /// DEPRECATED. Alias for ListEnabledApps.
    #[deprecated = "alias for ListEnabledApps"]
    ListActiveApps,

    /// List the ids of the Apps that are installed in the conductor, returning their information.
    /// If `status_filter` is `Some(_)`, it will return only the `Apps` with the specified status
    ///
    /// Will be responded to with an [`AdminResponse::AppsListed`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminResponse::AppsListed`]: enum.AdminResponse.html#variant.AppsListed
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    ListApps {
        status_filter: Option<AppStatusFilter>,
    },

    /// Changes the `App` specified by argument `installed_app_id` from a disabled state to an enabled state in the conductor,
    /// meaning that Zome calls can now be made and the `App` will be loaded on a reboot of the conductor.
    /// It is likely to want to call this after calling [`AdminRequest::InstallApp`], since a freshly
    /// installed `App` is not activated automatically.
    ///
    /// Will be responded to with an [`AdminResponse::AppEnabled`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminRequest::InstallApp`]: enum.AdminRequest.html#variant.InstallApp
    /// [`AdminResponse::AppEnabled`]: enum.AdminResponse.html#variant.AppEnabled
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    EnableApp {
        /// The InstalledAppId to enable
        installed_app_id: InstalledAppId,
    },

    /// DEPRECATED. Alias for EnableApp.
    #[deprecated = "alias for EnableApp"]
    ActivateApp {
        /// The InstalledAppId to enable
        installed_app_id: InstalledAppId,
    },

    /// Changes the `App` specified by argument `installed_app_id` from an enabled state to a disabled state in the conductor,
    /// meaning that Zome calls can no longer be made, and the `App` will not be loaded on a
    /// reboot of the conductor.
    ///
    /// Will be responded to with an [`AdminResponse::AppDisabled`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminResponse::AppDisabled`]: enum.AdminResponse.html#variant.AppDisabled
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    DisableApp {
        /// The InstalledAppId to disable
        installed_app_id: InstalledAppId,
    },

    /// DEPRECATED. Alias for DisableApp.
    #[deprecated = "alias for DisableApp"]
    DeactivateApp {
        /// The InstalledAppId to disable
        installed_app_id: InstalledAppId,
    },

    StartApp {
        /// The InstalledAppId to (re)start
        installed_app_id: InstalledAppId,
    },

    /// Open up a new websocket interface at the networking port
    /// (optionally) specified by argument `port` (or using any free port if argument `port` is `None`)
    /// over which you can then use the [`AppRequest`] API.
    /// Any active `App` will be callable via this interface.
    /// The successful [`AdminResponse::AppInterfaceAttached`] message will contain
    /// the port chosen by the conductor if `None` was passed.
    ///
    /// Will be responded to with an [`AdminResponse::AppInterfaceAttached`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminResponse::AppInterfaceAttached`]: enum.AdminResponse.html#variant.AppInterfaceAttached
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    AttachAppInterface {
        /// Optional port, use None to let the
        /// OS choose a free port
        port: Option<u16>,
    },

    /// List all the app interfaces currently attached with [`AttachAppInterface`].
    ListAppInterfaces,

    /// Dump the full state of the `Cell` specified by argument `cell_id`,
    /// including its chain, as a string containing JSON.
    ///
    /// Will be responded to with an [`AdminResponse::StateDumped`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    /// [`AdminResponse::StateDumped`]: enum.AdminResponse.html#variant.StateDumped
    DumpState {
        /// The `CellId` for which to dump state
        cell_id: Box<CellId>,
    },

    /// Add a list [AgentInfoSigned] to this conductor's peer store.
    /// This is another way of finding peers on a dht.
    ///
    /// This can be useful for testing.
    ///
    /// It is also helpful if you know other
    /// agents on the network and they can send you
    /// their agent info.
    AddAgentInfo {
        /// Vec of signed agent info to add to peer store
        agent_infos: Vec<AgentInfoSigned>,
    },

    /// Request the [AgentInfoSigned] stored in this conductor's
    /// peer store.
    ///
    /// You can:
    /// - Get all agent info by leaving cell id to None.
    /// - Get a specific agent info by setting the cell id.
    ///
    /// This is how you can send your agent info to another agent.
    /// It is also useful for testing across networks.
    RequestAgentInfo {
        /// Optionally choose a specific agent info
        cell_id: Option<CellId>,
    },
}

/// Represents the possible responses to an [`AdminRequest`]
/// and follows a general convention of `noun_verb` as opposed to
/// the `verb_noun` of `AdminRequest`.
///
/// Will serialize as an object with any contents of the enum on a key `data`
/// and the enum variant on a key `type`, e.g.
/// `{ type: 'app_interface_attached', data: { port: 4000 } }`
///
/// [`AdminRequest`]: enum.AdminRequest.html
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[cfg_attr(test, derive(Clone))]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum AdminResponse {
    /// Can occur in response to any [`AdminRequest`].
    ///
    /// There has been an error during the handling of the request.
    /// See [`ExternalApiWireError`] for variants.
    ///
    /// [`AdminRequest`]: enum.AdminRequest.html
    /// [`ExternalApiWireError`]: error/enum.ExternalApiWireError.html
    Error(ExternalApiWireError),

    /// The successful response to an [`AdminRequest::RegisterDna`]
    ///
    /// [`AdminRequest::RegisterDna`]: enum.AdminRequest.html#variant.RegisterDna
    DnaRegistered(DnaHash),

    /// The successful response to an [`AdminRequest::InstallApp`].
    ///
    /// The resulting [`InstalledAppInfo`] contains the App id,
    /// the [`CellNick`]s and, most usefully, the new [`CellId`]s
    /// of the newly installed `Dna`s. See the [`InstalledAppInfo`] docs for details.
    ///
    /// [`AdminRequest::InstallApp`]: enum.AdminRequest.html#variant.InstallApp
    /// [`InstalledAppInfo`]: ../../../holochain_types/app/struct.InstalledAppInfo.html
    /// [`CellNick`]: ../../../holochain_types/app/type.CellNick.html
    /// [`CellId`]: ../../../holochain_types/cell/struct.CellId.html
    AppInstalled(InstalledAppInfo),

    /// The successful response to an [`AdminRequest::InstallAppBundle`].
    ///
    /// The resulting [`InstalledAppInfo`] contains the App id,
    /// the [`CellNick`]s and, most usefully, the new [`CellId`]s
    /// of the newly installed `Dna`s. See the [`InstalledAppInfo`] docs for details.
    ///
    /// [`AdminRequest::InstallApp`]: enum.AdminRequest.html#variant.InstallApp
    /// [`InstalledAppInfo`]: ../../../holochain_types/app/struct.InstalledAppInfo.html
    /// [`CellNick`]: ../../../holochain_types/app/type.CellNick.html
    /// [`CellId`]: ../../../holochain_types/cell/struct.CellId.html
    AppBundleInstalled(InstalledAppInfo),

    /// The succesful response to an [`AdminRequest::UninstallApp`].
    ///
    /// It means the `App` was uninstalled successfully.
    ///
    /// [`AdminRequest::UninstallApp`]: enum.AdminRequest.html#variant.UninstallApp
    AppUninstalled,

    /// The successful response to an [`AdminRequest::CreateCloneCell`].
    ///
    /// The response contains the [`CellId`] of the newly created clone.
    ///
    /// [`AdminRequest::CreateCloneCell`]: enum.AdminRequest.html#variant.CreateCloneCell
    /// [`CellId`]: ../../../holochain_types/cell/struct.CellId.html
    CloneCellCreated(CellId),

    /// The succesful response to an [`AdminRequest::AddAdminInterfaces`].
    ///
    /// It means the `AdminInterface`s have successfully been added
    ///
    /// [`AdminRequest::AddAdminInterfaces`]: enum.AdminRequest.html#variant.AddAdminInterfaces
    AdminInterfacesAdded,

    /// The succesful response to an [`AdminRequest::GenerateAgentPubKey`].
    ///
    /// Contains a new `AgentPubKey` generated by the Keystore
    ///
    /// [`AdminRequest::GenerateAgentPubKey`]: enum.AdminRequest.html#variant.GenerateAgentPubKey
    AgentPubKeyGenerated(AgentPubKey),

    /// The successful response to an [`AdminRequest::ListDnas`].
    ///
    /// Contains a list of the hashes of all installed `Dna`s
    ///
    /// [`AdminRequest::ListDnas`]: enum.AdminRequest.html#variant.ListDnas
    DnasListed(Vec<DnaHash>),

    /// The succesful response to an [`AdminRequest::ListCellIds`].
    ///
    /// Contains a list of all the `Cell` ids in the conductor
    ///
    /// [`AdminRequest::ListCellIds`]: enum.AdminRequest.html#variant.ListCellIds
    CellIdsListed(Vec<CellId>),

    /// The succesful response to an [`AdminRequest::ListEnabledApps`].
    ///
    /// Contains a list of all the active `App` ids in the conductor
    ///
    /// [`AdminRequest::ListEnabledApps`]: enum.AdminRequest.html#variant.ListEnabledApps
    EnabledAppsListed(Vec<InstalledAppId>),

    #[deprecated = "alias for EnabledAppsListed"]
    ActiveAppsListed(Vec<InstalledAppId>),

    /// The succesful response to an [`AdminRequest::ListApps`].
    ///
    /// Contains a list of the `InstalledAppInfo` of the installed `Apps` in the conductor
    ///
    /// [`AdminRequest::ListApps`]: enum.AdminRequest.html#variant.ListApps
    AppsListed(Vec<InstalledAppInfo>),

    /// The succesful response to an [`AdminRequest::AttachAppInterface`].
    ///
    /// `AppInterfaceApi` successfully attached.
    /// Contains the port number that was selected (if not specified) by Holochain
    /// for running this App interface
    ///
    /// [`AdminRequest::AttachAppInterface`]: enum.AdminRequest.html#variant.AttachAppInterface
    AppInterfaceAttached {
        /// Networking port of the new `AppInterfaceApi`
        port: u16,
    },

    /// The list of attached app interfaces.
    AppInterfacesListed(Vec<u16>),

    /// The succesful response to an [`AdminRequest::EnableApp`].
    ///
    /// It means the `App` was enabled successfully. If it was possible to
    /// put the app in a Running state, it will be Running, otherwise it will
    /// be Paused.
    ///
    /// [`AdminRequest::EnableApp`]: enum.AdminRequest.html#variant.EnableApp
    AppEnabled {
        app: InstalledAppInfo,
        errors: Vec<(CellId, String)>,
    },

    #[deprecated = "alias for AppEnabled"]
    AppActivated {
        app: InstalledAppInfo,
        errors: Vec<(CellId, String)>,
    },

    /// The succesful response to an [`AdminRequest::DisableApp`].
    ///
    /// It means the `App` was disabled successfully.
    ///
    /// [`AdminRequest::DisableApp`]: enum.AdminRequest.html#variant.DisableApp
    AppDisabled,

    /// The succesful response to an [`AdminRequest::StartApp`].
    ///
    /// The boolean determines whether or not the was actually started.
    /// If false, it was because the app was in a disabled state, or the app
    /// failed to start.
    /// TODO: add reason why app couldn't start
    ///
    /// [`AdminRequest::StartApp`]: enum.AdminRequest.html#variant.StartApp
    AppStarted(bool),

    #[deprecated = "alias for AppDisabled"]
    AppDeactivated,

    /// The succesful response to an [`AdminRequest::DumpState`].
    ///
    /// The result contains a string of serialized JSON data which can be deserialized to access the
    /// full state dump, and inspect the source chain.
    ///
    /// [`AdminRequest::DumpState`]: enum.AdminRequest.html#variant.DumpState
    StateDumped(String),

    /// The succesful response to an [`AdminRequest::AddAgentInfo`].
    ///
    /// This means the agent info was successfully added to the peer store.
    ///
    /// [`AdminRequest::AddAgentInfo`]: enum.AdminRequest.html#variant.AddAgentInfo
    AgentInfoAdded,

    /// The succesful response to an [`AdminRequest::RequestAgentInfo`].
    ///
    /// This is all the agent info that was found for the request.
    ///
    /// [`AdminRequest::RequestAgentInfo`]: enum.AdminRequest.html#variant.RequestAgentInfo
    AgentInfoRequested(Vec<AgentInfoSigned>),
}

/// Error type that goes over the websocket wire.
/// This intends to be application developer facing
/// so it should be readable and relevant
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes, Clone)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum ExternalApiWireError {
    // TODO: B-01506 Constrain these errors so they are relevant to
    // application developers and what they would need
    // to react to using code (i.e. not just print)
    /// Any internal error
    InternalError(String),
    /// The input to the api failed to Deseralize
    Deserialization(String),
    /// The dna path provided was invalid
    DnaReadError(String),
    /// There was an error in the ribosome
    RibosomeError(String),
    /// Error activating app
    ActivateApp(String),
    /// The zome call is unauthorized
    ZomeCallUnauthorized(String),
    /// A countersigning session has failed.
    CountersigningSessionError(String),
}

impl ExternalApiWireError {
    /// Convert the error from the display.
    pub fn internal<T: std::fmt::Display>(e: T) -> Self {
        // Display format is used because
        // this version intended for users.
        ExternalApiWireError::InternalError(e.to_string())
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes, Clone)]
/// Filter for `ListApps`.
pub enum AppStatusFilter {
    Enabled,
    Disabled,
    Running,
    Stopped,
    Paused,
}
