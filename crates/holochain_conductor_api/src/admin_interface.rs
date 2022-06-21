use holo_hash::*;
use holochain_types::prelude::*;
use holochain_zome_types::cell::CellId;
use kitsune_p2p::agent_store::AgentInfoSigned;

use crate::{FullStateDump, InstalledAppInfo};

/// Represents the available conductor functions to call over an admin interface.
///
/// Enum variants follow a general convention of `verb_noun` as opposed to
/// the `noun_verb` of responses.
///
/// # Errors
///
/// Returns an [`AdminResponse::Error`] with a reason why the request failed.
// Expects a serialized object with any contents of the enum on a key `data`
// and the enum variant on a key `type`, e.g.
// `{ type: 'enable_app', data: { installed_app_id: 'test_app' } }`
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum AdminRequest {
    /// Set up and register one or more new admin interfaces
    /// as specified by a list of configurations.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::AdminInterfacesAdded`]
    AddAdminInterfaces(Vec<crate::config::AdminInterfaceConfig>),

    /// Register a DNA for later app installation.
    ///
    /// Stores the given DNA into the Holochain DNA database and returns the hash of it.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::DnaRegistered`]
    RegisterDna(Box<RegisterDnaPayload>),

    /// Clone a DNA (in the biological sense), thus creating a new `Cell`.
    ///
    /// Using the provided, already-registered DNA, create a new DNA with a unique
    /// ID and the specified properties, create a new cell from this cloned DNA,
    /// and add the cell to the specified app.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::CloneCellCreated`]
    CreateCloneCell(Box<CreateCloneCellPayload>),

    /// Install an app from a list of DNA paths.
    ///
    /// Triggers genesis to be run on all cells and to be stored.
    /// An app is intended for use by
    /// one and only one agent and for that reason it takes an `AgentPubKey` and
    /// installs all the DNAs with that `AgentPubKey` forming new Cells.
    /// See [`InstallAppPayload`] for full details on the configuration.
    ///
    /// Note that the new app will not be enabled automatically after installation
    /// and can be enabled by calling [`EnableApp`].
    ///
    /// # Returns
    ///
    /// [`AdminResponse::AppInstalled`]
    ///
    /// [`EnableApp`]: AdminRequest::EnableApp
    InstallApp(Box<InstallAppPayload>),

    /// Install an app using an [`AppBundle`].
    ///
    /// Triggers genesis to be run on all Cells and to be stored.
    /// An app is intended for use by
    /// one and only one Agent and for that reason it takes an `AgentPubKey` and
    /// installs all the DNAs with that `AgentPubKey`, forming new cells.
    /// See [`InstallAppBundlePayload`] for full details on the configuration.
    ///
    /// Note that the new app will not be enabled automatically after installation
    /// and can be enabled by calling [`EnableApp`].
    ///
    /// # Returns
    ///
    /// [`AdminResponse::AppInstalled`]
    ///
    /// [`EnableApp`]: AdminRequest::EnableApp
    InstallAppBundle(Box<InstallAppBundlePayload>),

    /// Uninstalls the app specified by argument `installed_app_id` from the conductor.
    ///
    /// The app will be removed from the list of installed apps, and any cells
    /// which were referenced only by this app will be disabled and removed, clearing up
    /// any persisted data.
    /// Cells which are still referenced by other installed apps will not be removed.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::AppUninstalled`]
    UninstallApp {
        /// The app ID to uninstall
        installed_app_id: InstalledAppId,
    },

    /// List the hashes of all installed DNAs.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::DnasListed`]
    ListDnas,

    /// Generate a new [`AgentPubKey`].
    ///
    /// # Returns
    ///
    /// [`AdminResponse::AgentPubKeyGenerated`]
    GenerateAgentPubKey,

    /// List all the cell IDs in the conductor.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::CellIdsListed`]
    ListCellIds,

    /// List the IDs of all enabled apps in the conductor.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::ActiveAppsListed`]
    ListEnabledApps,

    #[deprecated = "alias for ListEnabledApps"]
    ListActiveApps,

    /// List the apps and their information that are installed in the conductor.
    ///
    /// If `status_filter` is `Some(_)`, it will return only the apps with the specified status.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::AppsListed`]
    ListApps {
        /// An optional status to filter the list of apps by
        status_filter: Option<AppStatusFilter>,
    },

    /// Changes the specified app from a disabled to an enabled state in the conductor.
    ///
    /// It is likely to want to call this after calling [`AdminRequest::InstallApp`], since a freshly
    /// installed app is not enabled automatically. When an app is enabled,
    /// zomes can be called and it will be loaded on a reboot of the conductor.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::AppEnabled`]
    EnableApp {
        /// The app ID to enable
        installed_app_id: InstalledAppId,
    },

    #[deprecated = "alias for EnableApp"]
    ActivateApp { installed_app_id: InstalledAppId },

    /// Changes the specified app from an enabled to a disabled state in the conductor.
    ///
    /// When an app is disabled, zome calls can no longer be made, and the app will not be
    /// loaded on a reboot of the conductor.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::AppDisabled`]
    DisableApp {
        /// The app ID to disable
        installed_app_id: InstalledAppId,
    },

    #[deprecated = "alias for DisableApp"]
    DeactivateApp { installed_app_id: InstalledAppId },

    StartApp {
        /// The app ID to (re)start
        installed_app_id: InstalledAppId,
    },

    /// Open up a new websocket for processing [`AppRequest`]s.
    ///
    /// Any active app will be callable via the attached app interface.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::AppInterfaceAttached`]
    ///
    /// # Arguments
    ///
    /// Optionally a `port` parameter can be passed to this request. If it is `None`,
    /// a free port is chosen by the conductor.
    /// The response will contain the port chosen by the conductor if `None` was passed.
    ///
    /// [`AppRequest`]: super::AppRequest
    AttachAppInterface {
        /// Optional port number
        port: Option<u16>,
    },

    /// List all the app interfaces currently attached with [`AttachAppInterface`].
    ///
    /// # Returns
    ///
    /// [`AdminResponse::AppInterfacesListed`], a list of websocket ports that can
    /// process [`AppRequest`]s.
    ///
    /// [`AttachAppInterface`]: AdminRequest::AttachAppInterface
    /// [`AppRequest`]: super::AppRequest
    ListAppInterfaces,

    /// Dump the state of the cell specified by argument `cell_id`,
    /// including its chain, as a string containing JSON.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::StateDumped`]
    DumpState {
        /// The cell ID for which to dump state
        cell_id: Box<CellId>,
    },

    /// Dump the full state of the Cell specified by argument `cell_id`,
    /// including its chain and DHT shard, as a string containing JSON.
    ///
    /// **Warning**: this API call is subject to change, and will not be available to hApps.
    /// This is meant to be used by introspection tooling.
    ///
    /// Note that the response to this call can be very big, as it's requesting for
    /// the full database of the cell.
    ///
    /// Also note that while DHT ops about private entries will be returned (like `StoreCommit`),
    /// the entry in itself will be missing, as it's not actually stored publicly in the DHT shard.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::FullStateDumped`]
    DumpFullState {
        /// The cell ID for which to dump the state
        cell_id: Box<CellId>,
        /// The last seen DhtOp RowId, returned in the full dump state.
        /// Only DhtOps with RowId greater than the cursor will be returned.
        dht_ops_cursor: Option<u64>,
    },

    /// Dump the network metrics tracked by kitsune.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::NetworkMetricsDumped`]
    DumpNetworkMetrics {
        /// If set, limits the metrics dumped to a single DNA hash space.
        dna_hash: Option<DnaHash>,
    },

    /// Add a list of agents to this conductor's peer store.
    ///
    /// This is a way of shortcutting peer discovery and is useful for testing.
    ///
    /// It is also helpful if you know other
    /// agents on the network and they can send you
    /// their agent info.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::AgentInfoAdded`]
    AddAgentInfo {
        /// list of signed agent info to add to peer store
        agent_infos: Vec<AgentInfoSigned>,
    },

    /// Request the [`AgentInfoSigned`] stored in this conductor's
    /// peer store.
    ///
    /// You can:
    /// - Get all agent info by leaving `cell_id` to `None`.
    /// - Get a specific agent info by setting the `cell_id`.
    ///
    /// This is how you can send your agent info to another agent.
    /// It is also useful for testing across networks.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::AgentInfoRequested`]
    RequestAgentInfo {
        /// Optionally choose the agent info of a specific cell.
        cell_id: Option<CellId>,
    },

    /// Insert [`Commit`]s into the source chain of the [`CellId`].
    ///
    /// All commits must be authored and signed by the same agent.
    /// The [`DnaFile`] (but not necessarily the cell) must already be installed
    /// on this conductor.
    ///
    /// Care is needed when using this command as it can result in
    /// an invalid chain.
    /// Additionally, if conflicting source chain commits are
    /// inserted on different nodes, then the chain will be forked.
    ///
    /// If an invalid or forked chain is inserted
    /// and then pushed to the DHT, it can't be undone.
    ///
    /// Note that the cell does not need to exist to run this command.
    /// It is possible to insert commits into a source chain before
    /// the cell is created. This can be used to restore from backup.
    ///
    /// If the cell is installed, it is best to call [`AdminRequest::DisableApp`]
    /// before running this command, as otherwise the chain head may move.
    /// If `truncate` is true, the chain head is not checked and any new
    /// commits will be lost.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::CommitsAdded`]
    AddCommits {
        /// The cell that the commits are being inserted into.
        cell_id: CellId,
        /// If this is true then all commits in the source chain will be
        /// removed before the new commits are inserted.
        /// **Warning**: this cannot be undone. Use with care!
        ///
        /// If this is `false`, then the commits will be appended to the end
        /// of the source chain.
        truncate: bool,
        /// If this is `true`, then the commits will be validated before insertion.
        /// This is much slower but is useful for verifying the chain is valid.
        ///
        /// If this is `false`, then commits will be inserted as is.
        /// This could lead to an invalid chain.
        validate: bool,
        /// The commits to be inserted into the source chain.
        commits: Vec<Commit>,
    },
}

/// Represents the possible responses to an [`AdminRequest`]
/// and follows a general convention of `noun_verb` as opposed to
/// the `verb_noun` of `AdminRequest`.
///
/// Will serialize as an object with any contents of the enum on a key `data`
/// and the enum variant on a key `type`, e.g.
/// `{ type: 'app_interface_attached', data: { port: 4000 } }`
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[cfg_attr(test, derive(Clone))]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum AdminResponse {
    /// Can occur in response to any [`AdminRequest`].
    ///
    /// There has been an error during the handling of the request.
    Error(ExternalApiWireError),

    /// The successful response to an [`AdminRequest::RegisterDna`]
    DnaRegistered(DnaHash),

    /// The successful response to an [`AdminRequest::InstallApp`].
    ///
    /// The resulting [`InstalledAppInfo`] contains the app ID,
    /// the [`AppRoleId`]s and, most usefully, the new [`CellId`]s
    /// of the newly installed DNAs.
    AppInstalled(InstalledAppInfo),

    /// The successful response to an [`AdminRequest::InstallAppBundle`].
    ///
    /// The resulting [`InstalledAppInfo`] contains the app ID,
    /// the [`AppRoleId`]s and, most usefully, the new [`CellId`]s
    /// of the newly installed DNAs.
    AppBundleInstalled(InstalledAppInfo),

    /// The successful response to an [`AdminRequest::UninstallApp`].
    ///
    /// It means the app was uninstalled successfully.
    AppUninstalled,

    /// The successful response to an [`AdminRequest::CreateCloneCell`].
    ///
    /// The response contains the [`CellId`] of the newly created clone.
    CloneCellCreated(CellId),

    /// The successful response to an [`AdminRequest::AddAdminInterfaces`].
    ///
    /// It means the `AdminInterface`s have successfully been added.
    AdminInterfacesAdded,

    /// The successful response to an [`AdminRequest::GenerateAgentPubKey`].
    ///
    /// Contains a new [`AgentPubKey`] generated by the keystore.
    AgentPubKeyGenerated(AgentPubKey),

    /// The successful response to an [`AdminRequest::ListDnas`].
    ///
    /// Contains a list of the hashes of all installed DNAs.
    DnasListed(Vec<DnaHash>),

    /// The successful response to an [`AdminRequest::ListCellIds`].
    ///
    /// Contains a list of all the cell IDs in the conductor.
    CellIdsListed(Vec<CellId>),

    /// The successful response to an [`AdminRequest::ListEnabledApps`].
    ///
    /// Contains a list of all the active app IDs in the conductor.
    EnabledAppsListed(Vec<InstalledAppId>),

    #[deprecated = "alias for EnabledAppsListed"]
    ActiveAppsListed(Vec<InstalledAppId>),

    /// The successful response to an [`AdminRequest::ListApps`].
    ///
    /// Contains a list of the `InstalledAppInfo` of the installed apps in the conductor.
    AppsListed(Vec<InstalledAppInfo>),

    /// The successful response to an [`AdminRequest::AttachAppInterface`].
    ///
    /// `AppInterfaceApi` successfully attached.
    /// If no port was specified in the request, contains the port number that was
    /// selected by the conductor for running this app interface.
    AppInterfaceAttached {
        /// Networking port of the new `AppInterfaceApi`
        port: u16,
    },

    /// The list of attached app interfaces.
    AppInterfacesListed(Vec<u16>),

    /// The successful response to an [`AdminRequest::EnableApp`].
    ///
    /// It means the app was enabled successfully. If it was possible to
    /// put the app in a running state, it will be running, otherwise it will
    /// be paused.
    AppEnabled {
        app: InstalledAppInfo,
        errors: Vec<(CellId, String)>,
    },

    #[deprecated = "alias for AppEnabled"]
    AppActivated {
        app: InstalledAppInfo,
        errors: Vec<(CellId, String)>,
    },

    /// The successful response to an [`AdminRequest::DisableApp`].
    ///
    /// It means the app was disabled successfully.
    AppDisabled,

    /// The successful response to an [`AdminRequest::StartApp`].
    ///
    /// The boolean determines whether or not the app was actually started.
    /// If `false`, it was because the app was in a disabled state, or the app
    /// failed to start.
    /// TODO: add reason why app couldn't start
    AppStarted(bool),
    #[deprecated = "alias for AppDisabled"]
    AppDeactivated,

    /// The successful response to an [`AdminRequest::DumpState`].
    ///
    /// The result contains a string of serialized JSON data which can be deserialized to access the
    /// full state dump and inspect the source chain.
    StateDumped(String),

    /// The successful response to an [`AdminRequest::DumpFullState`].
    ///
    /// The result contains a string of serialized JSON data which can be deserialized to access the
    /// full state dump and inspect the source chain.
    ///
    /// Note that this result can be very big, as it's requesting the full database of the cell.
    FullStateDumped(FullStateDump),

    /// The successful result of a call to [`AdminRequest::DumpNetworkMetrics`].
    ///
    /// The string is a JSON blob of the metrics results.
    NetworkMetricsDumped(String),

    /// The successful response to an [`AdminRequest::AddAgentInfo`].
    ///
    /// This means the agent info was successfully added to the peer store.
    AgentInfoAdded,

    /// The successful response to an [`AdminRequest::RequestAgentInfo`].
    ///
    /// This is all the agent info that was found for the request.
    AgentInfoRequested(Vec<AgentInfoSigned>),

    /// The successful response to an [`AdminRequest::AddCommits`].
    CommitsAdded,
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
    /// The input to the API failed to deseralize.
    Deserialization(String),
    /// The DNA path provided was invalid.
    DnaReadError(String),
    /// There was an error in the ribosome.
    RibosomeError(String),
    /// Error activating app.
    ActivateApp(String),
    /// The zome call is unauthorized.
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
/// Filter for [`AdminRequest::ListApps`].
pub enum AppStatusFilter {
    Enabled,
    Disabled,
    Running,
    Stopped,
    Paused,
}
