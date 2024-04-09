use holo_hash::*;
use holochain_types::prelude::*;
use holochain_types::websocket::AllowedOrigins;
use holochain_zome_types::cell::CellId;
use kitsune_p2p_types::agent_info::AgentInfoSigned;

use crate::{AppInfo, FullStateDump, StorageInfo};

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

    /// Get the definition of a DNA.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::DnaDefinitionReturned`]
    GetDnaDefinition(Box<DnaHash>),

    /// Update coordinator zomes for an already installed DNA.
    ///
    /// Replaces any installed coordinator zomes with the same zome name.
    /// If the zome name doesn't exist then the coordinator zome is appended
    /// to the current list of coordinator zomes.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::CoordinatorsUpdated`]
    UpdateCoordinators(Box<UpdateCoordinatorsPayload>),

    /// Install an app using an [`AppBundle`].
    ///
    /// Triggers genesis to be run on all Cells and to be stored.
    /// An app is intended for use by
    /// one and only one Agent and for that reason it takes an `AgentPubKey` and
    /// installs all the DNAs with that `AgentPubKey`, forming new cells.
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

    /// List the IDs of all live cells currently running in the conductor.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::CellIdsListed`]
    ListCellIds,

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
    /// installed app is not enabled automatically. Once the app is enabled,
    /// zomes can be immediately called and it will also be loaded and enabled automatically on any reboot of the conductor.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::AppEnabled`]
    EnableApp {
        /// The app ID to enable
        installed_app_id: InstalledAppId,
    },

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

    /// Open up a new websocket for processing [`AppRequest`]s. Any active app will be
    /// callable via the attached app interface.
    ///
    /// **NB:** App interfaces are persisted when shutting down the conductor and are
    /// restored when restarting the conductor. Unused app interfaces are _not_ cleaned
    /// up. It is therefore recommended to reuse existing interfaces. They can be queried
    /// with the call [`AdminRequest::ListAppInterfaces`].
    ///
    /// # Returns
    ///
    /// [`AdminResponse::AppInterfaceAttached`]
    ///
    /// # Arguments
    ///
    /// Optionally a `port` parameter can be passed to this request. If it is `None`,
    /// a free port is chosen by the conductor.
    ///
    /// An `allowed_origins` parameter to control which origins are allowed to connect
    /// to the app interface.
    ///
    /// [`AppRequest`]: super::AppRequest
    AttachAppInterface {
        /// Optional port number
        port: Option<u16>,

        /// Allowed origins for this app interface.
        ///
        /// This should be one of:
        /// - A comma separated list of origins - `http://localhost:3000,http://localhost:3001`,
        /// - A single origin - `http://localhost:3000`,
        /// - Any origin - `*`
        ///
        /// Connections from any origin which is not permitted by this config will be rejected.
        allowed_origins: AllowedOrigins,

        /// Optionally bind this app interface to a specific installed app.
        ///
        /// If this is `None` then the interface can be used to establish a connection for any app.
        ///
        /// If this is `Some` then the interface will only accept connections for the specified app.
        /// Those connections will only be able to make calls to and receive signals from that app.
        installed_app_id: Option<InstalledAppId>,
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

    /// Dump the state of the conductor, including the in-memory representation
    /// and the persisted ConductorState, as JSON.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::ConductorStateDumped`]
    DumpConductorState,

    /// Dump the full state of the Cell specified by argument `cell_id`,
    /// including its chain and DHT shard, as a string containing JSON.
    ///
    /// **Warning**: this API call is subject to change, and will not be available to hApps.
    /// This is meant to be used by introspection tooling.
    ///
    /// Note that the response to this call can be very big, as it's requesting for
    /// the full database of the cell.
    ///
    /// Also note that while DHT ops about private entries will be returned (like `StoreRecord`),
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

    /// Dump raw json network statistics from the backend networking lib.
    DumpNetworkStats,

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
    /// [`AdminResponse::AgentInfo`]
    AgentInfo {
        /// Optionally choose the agent info of a specific cell.
        cell_id: Option<CellId>,
    },

    /// "Graft" [`Record`]s onto the source chain of the specified [`CellId`].
    ///
    /// The records must form a valid chain segment (ascending sequence numbers,
    /// and valid `prev_action` references). If the first record contains a `prev_action`
    /// which matches the existing records, then the new records will be "grafted" onto
    /// the existing chain at that point, and any other records following that point which do
    /// not match the new records will be removed.
    ///
    /// If this operation is called when there are no forks, the final state will also have
    /// no forks.
    ///
    /// **BEWARE** that this may result in the deletion of data! Any existing records which form
    /// a fork with respect to the new records will be deleted.
    ///
    /// All records must be authored and signed by the same agent.
    /// The [`DnaFile`] (but not necessarily the cell) must already be installed
    /// on this conductor.
    ///
    /// Care is needed when using this command as it can result in
    /// an invalid chain.
    /// Additionally, if conflicting source chain records are
    /// inserted on different nodes, then the chain will be forked.
    ///
    /// If an invalid or forked chain is inserted
    /// and then pushed to the DHT, it can't be undone.
    ///
    /// Note that the cell does not need to exist to run this command.
    /// It is possible to insert records into a source chain before
    /// the cell is created. This can be used to restore from backup.
    ///
    /// If the cell is installed, it is best to call [`AdminRequest::DisableApp`]
    /// before running this command, as otherwise the chain head may move.
    /// If `truncate` is true, the chain head is not checked and any new
    /// records will be lost.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::RecordsGrafted`]
    GraftRecords {
        /// The cell that the records are being inserted into.
        cell_id: CellId,
        /// If this is `true`, then the records will be validated before insertion.
        /// This is much slower but is useful for verifying the chain is valid.
        ///
        /// If this is `false`, then records will be inserted as is.
        /// This could lead to an invalid chain.
        validate: bool,
        /// The records to be inserted into the source chain.
        records: Vec<Record>,
    },

    /// Request capability grant for making zome calls.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::ZomeCallCapabilityGranted`]
    GrantZomeCallCapability(Box<GrantZomeCallCapabilityPayload>),

    /// Delete a clone cell that was previously disabled.
    ///
    /// # Returns
    ///
    /// [`AdminResponse::CloneCellDeleted`]
    DeleteCloneCell(Box<DeleteCloneCellPayload>),

    /// Info about storage used by apps
    StorageInfo,
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

    /// The successful response to an [`AdminRequest::GetDnaDefinition`]
    DnaDefinitionReturned(DnaDef),

    /// The successful response to an [`AdminRequest::UpdateCoordinators`]
    CoordinatorsUpdated,

    /// The successful response to an [`AdminRequest::InstallApp`].
    ///
    /// The resulting [`AppInfo`] contains the app ID,
    /// the [`RoleName`]s and, most usefully, [`CellInfo`](crate::CellInfo)s
    /// of the newly installed DNAs.
    AppInstalled(AppInfo),

    /// The successful response to an [`AdminRequest::UninstallApp`].
    ///
    /// It means the app was uninstalled successfully.
    AppUninstalled,

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

    /// The successful response to an [`AdminRequest::ListApps`].
    ///
    /// Contains a list of the `InstalledAppInfo` of the installed apps in the conductor.
    AppsListed(Vec<AppInfo>),

    /// The successful response to an [`AdminRequest::AttachAppInterface`].
    ///
    /// Contains the port number of the attached app interface.
    AppInterfaceAttached {
        /// Networking port of the new `AppInterfaceApi`
        port: u16,
    },

    /// The list of attached app interfaces.
    AppInterfacesListed(Vec<AppInterfaceInfo>),

    /// The successful response to an [`AdminRequest::EnableApp`].
    ///
    /// It means the app was enabled successfully. If it was possible to
    /// put the app in a running state, it will be running, otherwise it will
    /// be paused.
    AppEnabled {
        app: AppInfo,
        errors: Vec<(CellId, String)>,
    },

    /// The successful response to an [`AdminRequest::DisableApp`].
    ///
    /// It means the app was disabled successfully.
    AppDisabled,

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

    /// The successful response to an [`AdminRequest::DumpConductorState`].
    ///
    /// Simply a JSON serialized snapshot of `Conductor` and `ConductorState` from the `holochain` crate.
    ConductorStateDumped(String),

    /// The successful result of a call to [`AdminRequest::DumpNetworkMetrics`].
    ///
    /// The string is a JSON blob of the metrics results.
    NetworkMetricsDumped(String),

    /// The successful result of a call to [`AdminRequest::DumpNetworkStats`].
    ///
    /// The string is a raw JSON blob returned directly from the backend
    /// networking library.
    NetworkStatsDumped(String),

    /// The successful response to an [`AdminRequest::AddAgentInfo`].
    ///
    /// This means the agent info was successfully added to the peer store.
    AgentInfoAdded,

    /// The successful response to an [`AdminRequest::AgentInfo`].
    ///
    /// This is all the agent info that was found for the request.
    AgentInfo(Vec<AgentInfoSigned>),

    /// The successful response to an [`AdminRequest::GraftRecords`].
    RecordsGrafted,

    /// The successful response to an [`AdminRequest::GrantZomeCallCapability`].
    ZomeCallCapabilityGranted,

    /// The successful response to an [`AdminRequest::DeleteCloneCell`].
    CloneCellDeleted,

    /// The successful response to an [`AdminRequest::StorageInfo`].
    StorageInfo(StorageInfo),
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

/// Informational response for listing app interfaces.
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes, Clone)]
pub struct AppInterfaceInfo {
    /// The port that the app interface is listening on.
    pub port: u16,

    /// The allowed origins for this app interface.
    pub allowed_origins: AllowedOrigins,

    /// The optional association with a specific installed app.
    pub installed_app_id: Option<InstalledAppId>,
}

#[test]
fn admin_request_serialization() {
    use rmp_serde::Deserializer;

    // make sure requests are serialized as expected
    let request = AdminRequest::DisableApp {
        installed_app_id: "some_id".to_string(),
    };
    let serialized_request = holochain_serialized_bytes::encode(&request).unwrap();
    assert_eq!(
        serialized_request,
        vec![
            130, 164, 116, 121, 112, 101, 129, 171, 100, 105, 115, 97, 98, 108, 101, 95, 97, 112,
            112, 192, 164, 100, 97, 116, 97, 129, 176, 105, 110, 115, 116, 97, 108, 108, 101, 100,
            95, 97, 112, 112, 95, 105, 100, 167, 115, 111, 109, 101, 95, 105, 100
        ]
    );

    let json_expected = r#"{"type":{"disable_app":null},"data":{"installed_app_id":"some_id"}}"#;
    let mut deserializer = Deserializer::new(&*serialized_request);
    let json_value: serde_json::Value = Deserialize::deserialize(&mut deserializer).unwrap();
    let json_actual = serde_json::to_string(&json_value).unwrap();

    assert_eq!(json_actual, json_expected);

    // make sure responses are serialized as expected
    let response = AdminResponse::Error(ExternalApiWireError::RibosomeError(
        "error_text".to_string(),
    ));
    let serialized_response = holochain_serialized_bytes::encode(&response).unwrap();
    assert_eq!(
        serialized_response,
        vec![
            130, 164, 116, 121, 112, 101, 129, 165, 101, 114, 114, 111, 114, 192, 164, 100, 97,
            116, 97, 130, 164, 116, 121, 112, 101, 129, 174, 114, 105, 98, 111, 115, 111, 109, 101,
            95, 101, 114, 114, 111, 114, 192, 164, 100, 97, 116, 97, 170, 101, 114, 114, 111, 114,
            95, 116, 101, 120, 116
        ]
    );

    let json_expected =
        r#"{"type":{"error":null},"data":{"type":{"ribosome_error":null},"data":"error_text"}}"#;
    let mut deserializer = Deserializer::new(&*serialized_response);
    let json_value: serde_json::Value = Deserialize::deserialize(&mut deserializer).unwrap();
    let json_actual = serde_json::to_string(&json_value).unwrap();

    assert_eq!(json_actual, json_expected);
}
