use crate::{AppAuthenticationToken, ExternalApiWireError};
use holo_hash::AgentPubKey;
use holochain_keystore::LairResult;
use holochain_keystore::MetaLairClient;
use holochain_types::prelude::*;
use indexmap::IndexMap;
use kitsune_p2p_types::fetch_pool::FetchPoolInfo;

/// Represents the available conductor functions to call over an app interface
/// and will result in a corresponding [`AppResponse`] message being sent back over the
/// interface connection.
///
/// # Errors
///
/// Returns an [`AppResponse::Error`] with a reason why the request failed.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum AppRequest {
    /// Get info about the app that you are connected to, including info about each cell installed
    /// by this app.
    ///
    /// # Returns
    ///
    /// [`AppResponse::AppInfo`]
    AppInfo,

    /// Call a zome function.
    ///
    /// The payload to this call is composed of the serialized [`ZomeCallParams`] as bytes
    /// and the provenance's signature.
    ///
    /// Serialization must be performed with MessagePack. The resulting bytes are hashed using the
    /// SHA2 512-bit algorithm, and the hash is signed with the provenance's private ed25519 key.
    /// The hash is not included in the call's payload.
    ///
    /// # Returns
    ///
    /// [`AppResponse::ZomeCalled`] Indicates the zome call was deserialized successfully. If the
    /// call was authorized, the response yields the return value of the zome function as MessagePack
    /// encoded bytes. The bytes can be deserialized to the expected return type.
    ///
    /// This response is also returned when authorization of the zome call failed because of an
    /// invalid signature, capability grant or nonce.
    ///
    /// # Errors
    ///
    /// [`SerializedBytesError`] is returned when the serialized bytes could not be deserialized
    /// to the expected [`ZomeCallParams`].
    CallZome(Box<ZomeCallParamsSigned>),

    /// Get the state of a countersigning session.
    ///
    /// # Returns
    ///
    /// [`AppResponse::CountersigningSessionState`]
    ///
    /// # Errors
    ///
    /// [`CountersigningError::WorkspaceDoesNotExist`] likely indicates that an invalid cell id was
    /// passed in to the call.
    #[cfg(feature = "unstable-countersigning")]
    GetCountersigningSessionState(Box<CellId>),

    /// Abandon an unresolved countersigning session.
    ///
    /// If the current session has not been resolved automatically, it can be forcefully abandoned.
    /// A condition for this call to succeed is that at least one attempt has been made to resolve
    /// it automatically.
    ///
    /// # Returns
    ///
    /// [`AppResponse::CountersigningSessionAbandoned`]
    ///
    /// The session is marked for abandoning and the countersigning workflow was triggered. The session
    /// has not been abandoned yet.
    ///
    /// Upon successful abandoning the system signal [`SystemSignal::AbandonedCountersigning`] will
    /// be emitted and the session removed from state, so that [`AppRequest::GetCountersigningSessionState`]
    /// would return `None`.
    ///
    /// In the countersigning workflow it will first be attempted to resolve the session with incoming
    /// signatures of the countersigned entries, before force-abandoning the session. In a very rare event
    /// it could happen that in just the moment where the [`AppRequest::AbandonCountersigningSession`]
    /// is made, signatures for this session come in. If they are valid, the session will be resolved and
    /// published as usual. Should they be invalid, however, the flag to abandon the session is erased.
    /// In such cases this request can be retried until the session has been abandoned successfully.
    ///
    /// # Errors
    ///
    /// [`CountersigningError::WorkspaceDoesNotExist`] likely indicates that an invalid cell id was
    /// passed in to the call.
    ///
    /// [`CountersigningError::SessionNotFound`] when no ongoing session could be found for the provided
    /// cell id.
    ///
    /// [`CountersigningError::SessionNotUnresolved`] when an attempt to resolve the session
    /// automatically has not been made.
    #[cfg(feature = "unstable-countersigning")]
    AbandonCountersigningSession(Box<CellId>),

    /// Publish an unresolved countersigning session.
    ///
    /// If the current session has not been resolved automatically, it can be forcefully published.
    /// A condition for this call to succeed is that at least one attempt has been made to resolve
    /// it automatically.
    ///
    /// # Returns
    ///
    /// [`AppResponse::PublishCountersigningSessionTriggered`]
    ///
    /// The session is marked for publishing and the countersigning workflow was triggered. The session
    /// has not been published yet.
    ///
    /// Upon successful publishing the system signal [`SystemSignal::SuccessfulCountersigning`] will
    /// be emitted and the session removed from state, so that [`AppRequest::GetCountersigningSessionState`]
    /// would return `None`.
    ///
    /// In the countersigning workflow it will first be attempted to resolve the session with incoming
    /// signatures of the countersigned entries, before force-publishing the session. In a very rare event
    /// it could happen that in just the moment where the [`AppRequest::PublishCountersigningSession`]
    /// is made, signatures for this session come in. If they are valid, the session will be resolved and
    /// published as usual. Should they be invalid, however, the flag to publish the session is erased.
    /// In such cases this request can be retried until the session has been published successfully.
    ///
    /// # Errors
    ///
    /// [`CountersigningError::WorkspaceDoesNotExist`] likely indicates that an invalid cell id was
    /// passed in to the call.
    ///
    /// [`CountersigningError::SessionNotFound`] when no ongoing session could be found for the provided
    /// cell id.
    ///
    /// [`CountersigningError::SessionNotUnresolved`] when an attempt to resolve the session
    /// automatically has not been made.
    #[cfg(feature = "unstable-countersigning")]
    PublishCountersigningSession(Box<CellId>),

    /// Clone a DNA (in the biological sense), thus creating a new `Cell`.
    ///
    /// Using the provided, already-registered DNA, create a new DNA with a unique
    /// ID and the specified properties, create a new cell from this cloned DNA,
    /// and add the cell to the specified app.
    ///
    /// # Returns
    ///
    /// [`AppResponse::CloneCellCreated`]
    CreateCloneCell(Box<CreateCloneCellPayload>),

    /// Disable a clone cell.
    ///
    /// Providing a [`CloneId`] or [`CellId`], disable an existing clone cell.
    /// When the clone cell exists, it is disabled and can not be called any
    /// longer. If it doesn't exist, the call is a no-op.
    ///
    /// # Returns
    ///
    /// [`AppResponse::CloneCellDisabled`] if the clone cell existed
    /// and has been disabled.
    DisableCloneCell(Box<DisableCloneCellPayload>),

    /// Enable a clone cell that was previously disabled.
    ///
    /// # Returns
    ///
    /// [`AppResponse::CloneCellEnabled`]
    EnableCloneCell(Box<EnableCloneCellPayload>),

    /// Info about networking processes
    ///
    /// # Returns
    ///
    /// [`AppResponse::NetworkInfo`]
    NetworkInfo(Box<NetworkInfoRequestPayload>),

    /// List all host functions available to wasm on this conductor.
    ///
    /// # Returns
    ///
    /// [`AppResponse::ListWasmHostFunctions`]
    ListWasmHostFunctions,

    /// Provide the membrane proofs for this app, if this app was installed
    /// using `allow_deferred_memproofs` and memproofs were not provided at
    /// installation time.
    ///
    /// # Returns
    ///
    /// [`AppResponse::Ok`]
    ProvideMemproofs(MemproofMap),

    /// Enable the app, only in special circumstances.
    /// Can only be called while the app is in the `Disabled(NotStartedAfterProvidingMemproofs)` state.
    /// Cannot be used to enable the app if it's in any other state, or Disabled for any other reason.
    ///
    /// # Returns
    ///
    /// [`AppResponse::Ok`]
    EnableApp,
    //
    // TODO: implement after DPKI lands
    // /// Replace the agent key associated with this app with a new one.
    // /// The new key will be created using the same method which is used
    // /// when installing an app with no agent key provided.
    // ///
    // /// This method is only available if this app was installed using `allow_deferred_memproofs`,
    // /// and can only be called before [`AppRequest::ProvideMemproofs`] has been called.
    // /// Until then, it can be called as many times as needed.
    // ///
    // /// # Returns
    // ///
    // /// [`AppResponse::AppAgentKeyRotated`]
    // RotateAppAgentKey,
}

/// Represents the possible responses to an [`AppRequest`].
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum AppResponse {
    /// Can occur in response to any [`AppRequest`].
    ///
    /// There has been an error during the handling of the request.
    Error(ExternalApiWireError),

    /// The successful response to an [`AppRequest::AppInfo`].
    ///
    /// Option will be `None` if there is no installed app with the given `installed_app_id`.
    AppInfo(Option<AppInfo>),

    /// The successful response to an [`AppRequest::CallZome`].
    ///
    /// Note that [`ExternIO`] is simply a structure of [`struct@SerializedBytes`], so the client will have
    /// to decode this response back into the data provided by the zome using a [msgpack] library to utilize it.
    ///
    /// [msgpack]: https://msgpack.org/
    ZomeCalled(Box<ExternIO>),

    /// The successful response to an [`AppRequest::GetCountersigningSessionState`].
    #[cfg(feature = "unstable-countersigning")]
    CountersigningSessionState(Box<Option<CountersigningSessionState>>),

    /// The successful response to an [`AppRequest::AbandonCountersigningSession`].
    #[cfg(feature = "unstable-countersigning")]
    CountersigningSessionAbandoned,

    /// The successful response to an [`AppRequest::PublishCountersigningSession`].
    #[cfg(feature = "unstable-countersigning")]
    PublishCountersigningSessionTriggered,

    /// The successful response to an [`AppRequest::CreateCloneCell`].
    ///
    /// The response contains the created clone [`ClonedCell`].
    CloneCellCreated(ClonedCell),

    /// The successful response to an [`AppRequest::DisableCloneCell`].
    ///
    /// An existing clone cell has been disabled.
    CloneCellDisabled,

    /// The successful response to an [`AppRequest::EnableCloneCell`].
    ///
    /// A previously disabled clone cell has been enabled. The [`ClonedCell`]
    /// is returned.
    CloneCellEnabled(ClonedCell),

    /// NetworkInfo is returned
    NetworkInfo(Vec<NetworkInfo>),

    /// All the wasm host functions supported by this conductor.
    ListWasmHostFunctions(Vec<String>),

    /// The app agent key as been rotated, and the new key is returned.
    AppAgentKeyRotated(AgentPubKey),

    /// Operation successful, no payload.
    Ok,
}

/// The data provided over an app interface in order to make a zome call.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ZomeCallParamsSigned {
    /// Bytes of the serialized zome call payload that consists of all fields of the
    /// [`ZomeCallParams`].
    pub bytes: ExternIO,
    /// Signature by the provenance of the call, signing the bytes of the zome call payload.
    pub signature: Signature,
}

impl ZomeCallParamsSigned {
    pub fn new(bytes: Vec<u8>, signature: Signature) -> Self {
        Self {
            bytes: ExternIO::from(bytes),
            signature,
        }
    }

    pub async fn try_from_params(
        keystore: &MetaLairClient,
        params: ZomeCallParams,
    ) -> LairResult<Self> {
        let (bytes, bytes_hash) = params.serialize_and_hash().map_err(|e| e.to_string())?;
        let signature = params
            .provenance
            .sign_raw(keystore, bytes_hash.into())
            .await?;
        Ok(Self::new(bytes, signature))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum CellInfo {
    /// Cells provisioned at app installation as defined in the bundle.
    Provisioned(ProvisionedCell),

    // Cells created at runtime by cloning provisioned cells.
    Cloned(ClonedCell),

    /// Potential cells with deferred installation as defined in the bundle.
    /// Not yet implemented.
    Stem(StemCell),
}

impl CellInfo {
    pub fn new_provisioned(cell_id: CellId, dna_modifiers: DnaModifiers, name: String) -> Self {
        Self::Provisioned(ProvisionedCell {
            cell_id,
            dna_modifiers,
            name,
        })
    }

    pub fn new_cloned(
        cell_id: CellId,
        clone_id: CloneId,
        original_dna_hash: DnaHash,
        dna_modifiers: DnaModifiers,
        name: String,
        enabled: bool,
    ) -> Self {
        Self::Cloned(ClonedCell {
            cell_id,
            clone_id,
            original_dna_hash,
            dna_modifiers,
            name,
            enabled,
        })
    }
}

/// Cell whose instantiation has been deferred.
/// Not yet implemented.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StemCell {
    /// The hash of the DNA that this cell would be instantiated from
    pub original_dna_hash: DnaHash,
    /// The DNA modifiers that will be used when instantiating the cell
    pub dna_modifiers: DnaModifiers,
    /// An optional name to override the cell's bundle name when instantiating
    pub name: Option<String>,
}

/// Provisioned cell, a cell instantiated from a DNA on app installation.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProvisionedCell {
    /// The cell's identifying data
    pub cell_id: CellId,
    /// The DNA modifiers that were used to instantiate the cell
    pub dna_modifiers: DnaModifiers,
    /// The name the cell was instantiated with
    pub name: String,
}

/// Info about an installed app, returned as part of [`AppResponse::AppInfo`]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct AppInfo {
    /// The unique identifier for an installed app in this conductor
    pub installed_app_id: InstalledAppId,
    /// Info about the cells installed in this app. Lists of cells are ordered
    /// and contain first the provisioned cell, then enabled clone cells and
    /// finally disabled clone cells.
    pub cell_info: IndexMap<RoleName, Vec<CellInfo>>,
    /// The app's current status, in an API-friendly format
    pub status: AppInfoStatus,
    /// The app's agent pub key.
    pub agent_pub_key: AgentPubKey,
    /// The original AppManifest used to install the app, which can also be used to
    /// install the app again under a new agent.
    pub manifest: AppManifest,
    /// The timestamp when this app was installed.
    pub installed_at: Timestamp,
}

impl AppInfo {
    pub fn from_installed_app(
        app: &InstalledApp,
        dna_definitions: &IndexMap<CellId, DnaDefHashed>,
    ) -> Self {
        let installed_app_id = app.id().clone();
        let status = app.status().clone().into();
        let agent_pub_key = app.agent_key().to_owned();
        let mut manifest = app.manifest().clone();
        let installed_at = *app.installed_at();

        let mut cell_info: IndexMap<RoleName, Vec<CellInfo>> = IndexMap::new();
        app.roles().iter().for_each(|(role_name, role_assignment)| {
            // create a vector with info of all cells for this role
            let mut cell_info_for_role: Vec<CellInfo> = Vec::new();

            // push the base cell to the vector of cell infos
            if let Some(provisioned_dna_hash) = role_assignment.provisioned_dna_hash() {
                let provisioned_cell_id =
                    CellId::new(provisioned_dna_hash.clone(), agent_pub_key.clone());
                if let Some(dna_def) = dna_definitions.get(&provisioned_cell_id) {
                    // TODO: populate `enabled` with cell state once it is implemented for a base cell
                    let cell_info = CellInfo::new_provisioned(
                        provisioned_cell_id.clone(),
                        dna_def.modifiers.to_owned(),
                        dna_def.name.to_owned(),
                    );
                    cell_info_for_role.push(cell_info);

                    // Update the manifest with the installed hash
                    match &mut manifest {
                        AppManifest::V1(manifest) => {
                            if let Some(role) =
                                manifest.roles.iter_mut().find(|r| r.name == *role_name)
                            {
                                role.dna.installed_hash = Some(dna_def.hash.clone().into());
                            }
                        }
                    }
                } else {
                    tracing::error!(
                        "no DNA definition found for cell id {}",
                        provisioned_cell_id
                    );
                }
            } else {
                // no provisioned cell, thus there must be a deferred cell
                // this is not implemented as of now
                unimplemented!()
            };

            // push enabled clone cells to the vector of cell infos
            if let Some(clone_cells) = app.clone_cells_for_role_name(role_name) {
                clone_cells.for_each(|(clone_id, cell_id)| {
                    if let Some(dna_def) = dna_definitions.get(&cell_id) {
                        let cell_info = CellInfo::new_cloned(
                            cell_id,
                            clone_id.to_owned(),
                            dna_def.hash.to_owned(),
                            dna_def.modifiers.to_owned(),
                            dna_def.name.to_owned(),
                            true,
                        );
                        cell_info_for_role.push(cell_info);
                    } else {
                        tracing::error!("no DNA definition found for cell id {}", cell_id);
                    }
                });
            }

            // push disabled clone cells to the vector of cell infos
            if let Some(clone_cells) = app.disabled_clone_cells_for_role_name(role_name) {
                clone_cells.for_each(|(clone_id, cell_id)| {
                    if let Some(dna_def) = dna_definitions.get(&cell_id) {
                        let cell_info = CellInfo::new_cloned(
                            cell_id,
                            clone_id.to_owned(),
                            dna_def.hash.to_owned(),
                            dna_def.modifiers.to_owned(),
                            dna_def.name.to_owned(),
                            false,
                        );
                        cell_info_for_role.push(cell_info);
                    } else {
                        tracing::error!("no DNA definition found for cell id {}", cell_id);
                    }
                });
            }

            cell_info.insert(role_name.clone(), cell_info_for_role);
        });

        Self {
            installed_app_id,
            cell_info,
            status,
            agent_pub_key,
            manifest,
            installed_at,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
/// The parameters to revoke an agent for an app.
pub struct RevokeAgentKeyPayload {
    pub agent_key: AgentPubKey,
    pub app_id: InstalledAppId,
}

/// A flat, slightly more API-friendly representation of [`AppInfo`]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum AppInfoStatus {
    Paused { reason: PausedAppReason },
    Disabled { reason: DisabledAppReason },
    Running,
    AwaitingMemproofs,
}

impl From<AppStatus> for AppInfoStatus {
    fn from(i: AppStatus) -> Self {
        match i {
            AppStatus::Running => AppInfoStatus::Running,
            AppStatus::Disabled(reason) => AppInfoStatus::Disabled { reason },
            AppStatus::Paused(reason) => AppInfoStatus::Paused { reason },
            AppStatus::AwaitingMemproofs => AppInfoStatus::AwaitingMemproofs,
        }
    }
}

impl From<AppInfoStatus> for AppStatus {
    fn from(i: AppInfoStatus) -> Self {
        match i {
            AppInfoStatus::Running => AppStatus::Running,
            AppInfoStatus::Disabled { reason } => AppStatus::Disabled(reason),
            AppInfoStatus::Paused { reason } => AppStatus::Paused(reason),
            AppInfoStatus::AwaitingMemproofs => AppStatus::AwaitingMemproofs,
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct NetworkInfo {
    pub fetch_pool_info: FetchPoolInfo,
    pub current_number_of_peers: u32,
    pub arc_size: f64,
    pub total_network_peers: u32,
    pub bytes_since_last_time_queried: u64,
    pub completed_rounds_since_last_time_queried: u32,
}

/// The request payload that should be sent in a [`holochain_websocket::WireMessage::Authenticate`]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct AppAuthenticationRequest {
    /// The authentication token that was provided by the conductor when [`crate::admin_interface::AdminRequest::IssueAppAuthenticationToken`] was called.
    pub token: AppAuthenticationToken,
}

#[cfg(test)]
mod tests {
    use crate::{AppInfoStatus, AppRequest, AppResponse};
    use holochain_types::app::{AppStatus, DisabledAppReason, PausedAppReason};
    use serde::Deserialize;

    #[test]
    fn app_request_serialization() {
        use rmp_serde::Deserializer;

        // make sure requests are serialized as expected
        let request = AppRequest::AppInfo;
        let serialized_request = holochain_serialized_bytes::encode(&request).unwrap();
        assert_eq!(
            serialized_request,
            vec![129, 164, 116, 121, 112, 101, 168, 97, 112, 112, 95, 105, 110, 102, 111]
        );

        let json_expected = r#"{"type":"app_info"}"#;
        let mut deserializer = Deserializer::new(&*serialized_request);
        let json_value: serde_json::Value = Deserialize::deserialize(&mut deserializer).unwrap();
        let json_actual = serde_json::to_string(&json_value).unwrap();

        assert_eq!(json_actual, json_expected);

        // make sure responses are serialized as expected
        let response = AppResponse::ListWasmHostFunctions(vec![
            "host_fn_1".to_string(),
            "host_fn_2".to_string(),
        ]);
        let serialized_response = holochain_serialized_bytes::encode(&response).unwrap();
        assert_eq!(
            serialized_response,
            vec![
                130, 164, 116, 121, 112, 101, 184, 108, 105, 115, 116, 95, 119, 97, 115, 109, 95,
                104, 111, 115, 116, 95, 102, 117, 110, 99, 116, 105, 111, 110, 115, 165, 118, 97,
                108, 117, 101, 146, 169, 104, 111, 115, 116, 95, 102, 110, 95, 49, 169, 104, 111,
                115, 116, 95, 102, 110, 95, 50
            ]
        );

        let json_expected =
            r#"{"type":"list_wasm_host_functions","value":["host_fn_1","host_fn_2"]}"#;
        let mut deserializer = Deserializer::new(&*serialized_response);
        let json_value: serde_json::Value = Deserialize::deserialize(&mut deserializer).unwrap();
        let json_actual = serde_json::to_string(&json_value).unwrap();

        assert_eq!(json_actual, json_expected);
    }

    #[test]
    fn status_serialization() {
        use serde_json;

        let status: AppInfoStatus =
            AppStatus::Disabled(DisabledAppReason::Error("because".into())).into();

        assert_eq!(
            serde_json::to_string(&status).unwrap(),
            "{\"type\":\"disabled\",\"value\":{\"reason\":{\"type\":\"error\",\"value\":\"because\"}}}"
        );

        let status: AppInfoStatus =
            AppStatus::Paused(PausedAppReason::Error("because".into())).into();

        assert_eq!(
            serde_json::to_string(&status).unwrap(),
            "{\"type\":\"paused\",\"value\":{\"reason\":{\"type\":\"error\",\"value\":\"because\"}}}",
        );

        let status: AppInfoStatus = AppStatus::Disabled(DisabledAppReason::User).into();

        assert_eq!(
            serde_json::to_string(&status).unwrap(),
            "{\"type\":\"disabled\",\"value\":{\"reason\":{\"type\":\"user\"}}}",
        );
    }
}
