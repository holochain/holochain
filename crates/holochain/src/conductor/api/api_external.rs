use super::error::{
    ConductorApiError, ConductorApiResult, ExternalApiWireError, SerializationError,
};
use crate::conductor::{
    config::AdminInterfaceConfig,
    interface::error::{InterfaceError, InterfaceResult},
    ConductorHandle,
};
use holo_hash::*;
use holochain_serialized_bytes::prelude::*;
use holochain_types::{
    cell::CellHandle,
    dna::{DnaFile, Properties},
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
};
use std::path::PathBuf;
use tracing::*;

/// A trait that unifies both the admin and app interfaces
#[async_trait::async_trait]
pub trait InterfaceApi: 'static + Send + Sync + Clone {
    /// Which request is being made
    type ApiRequest: TryFrom<SerializedBytes, Error = SerializedBytesError> + Send + Sync;
    /// Which response is sent to the above request
    type ApiResponse: TryInto<SerializedBytes, Error = SerializedBytesError> + Send + Sync;
    /// Handle a request on this API
    async fn handle_request(
        &self,
        request: Result<Self::ApiRequest, SerializedBytesError>,
    ) -> InterfaceResult<Self::ApiResponse>;
}

/// A trait for the interface that a Conductor exposes to the outside world to use for administering the conductor.
/// This trait has a one mock implementation and one "Real" implementation
#[async_trait::async_trait]
pub trait AdminInterfaceApi: 'static + Send + Sync + Clone {
    /// Call an admin function to modify this Conductor's behavior
    async fn handle_admin_request_inner(
        &self,
        request: AdminRequest,
    ) -> ConductorApiResult<AdminResponse>;

    // -- provided -- //

    /// Route the request to be handled
    async fn handle_admin_request(&self, request: AdminRequest) -> AdminResponse {
        let res = self.handle_admin_request_inner(request).await;

        match res {
            Ok(response) => response,
            Err(e) => AdminResponse::Error(e.into()),
        }
    }
}
/// The interface that a Conductor exposes to the outside world.
#[async_trait::async_trait]
pub trait AppInterfaceApi: 'static + Send + Sync + Clone {
    /// Invoke a zome function on any cell in this conductor.
    async fn invoke_zome(
        &self,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse>;

    // -- provided -- //

    /// Routes the [AppRequest] to the [AppResponse]
    async fn handle_request(&self, request: AppRequest) -> AppResponse {
        let res: ConductorApiResult<AppResponse> = async move {
            match request {
                AppRequest::ZomeInvocationRequest { request } => {
                    Ok(AppResponse::ZomeInvocationResponse {
                        response: Box::new(self.invoke_zome(*request).await?),
                    })
                }
                _ => unimplemented!(),
            }
        }
        .await;

        match res {
            Ok(response) => response,
            Err(e) => AppResponse::Error {
                debug: format!("{:?}", e),
            },
        }
    }
}

/// The admin interface that external connections
/// can use to make requests to the conductor
/// The concrete (non-mock) implementation of the AdminInterfaceApi
#[derive(Clone)]
pub struct RealAdminInterfaceApi {
    /// Mutable access to the Conductor
    conductor_handle: ConductorHandle,

    /// Needed to spawn an App interface
    app_api: RealAppInterfaceApi,
}

impl RealAdminInterfaceApi {
    pub(crate) fn new(conductor_handle: ConductorHandle) -> Self {
        let app_api = RealAppInterfaceApi::new(conductor_handle.clone());
        RealAdminInterfaceApi {
            conductor_handle,
            app_api,
        }
    }
}

#[async_trait::async_trait]
impl AdminInterfaceApi for RealAdminInterfaceApi {
    async fn handle_admin_request_inner(
        &self,
        request: AdminRequest,
    ) -> ConductorApiResult<AdminResponse> {
        use AdminRequest::*;
        match request {
            Start(_cell_handle) => unimplemented!(),
            Stop(_cell_handle) => unimplemented!(),
            AddAdminInterfaces(configs) => Ok(AdminResponse::AdminInterfacesAdded(
                self.conductor_handle
                    .add_admin_interfaces_via_handle(self.conductor_handle.clone(), configs)
                    .await?,
            )),
            InstallDna(dna_path, properties) => {
                trace!(?dna_path);
                let dna = read_parse_dna(dna_path, properties).await?;
                self.conductor_handle.install_dna(dna).await?;
                Ok(AdminResponse::DnaInstalled)
            }
            ListDnas => {
                let dna_list = self.conductor_handle.list_dnas().await?;
                Ok(AdminResponse::ListDnas(dna_list))
            }
            GenerateAgentPubKey => {
                let agent_pub_key = self
                    .conductor_handle
                    .keystore()
                    .clone()
                    .generate_sign_keypair_from_pure_entropy()
                    .await?;
                Ok(AdminResponse::GenerateAgentPubKey(agent_pub_key))
            }
            ListAgentPubKeys => {
                let pub_key_list = self
                    .conductor_handle
                    .keystore()
                    .clone()
                    .list_sign_keys()
                    .await?;
                Ok(AdminResponse::ListAgentPubKeys(pub_key_list))
            }
        }
    }
}

/// Reads the [Dna] from disk and parses to [SerializedBytes]
async fn read_parse_dna(
    dna_path: PathBuf,
    properties: Option<serde_json::Value>,
) -> ConductorApiResult<DnaFile> {
    let dna: UnsafeBytes = tokio::fs::read(dna_path)
        .await
        .map_err(|e| ConductorApiError::DnaReadError(format!("{:?}", e)))?
        .into();
    let dna = SerializedBytes::from(dna);
    let mut dna: DnaFile = dna.try_into().map_err(SerializationError::from)?;
    if let Some(properties) = properties {
        let properties = Properties::new(properties);
        let (mut tmp_dna, tmp_wasm): (
            holochain_types::dna::DnaDef,
            Vec<holochain_types::dna::wasm::DnaWasm>,
        ) = dna.into();
        tmp_dna.properties = properties.try_into().map_err(SerializationError::from)?;
        dna = DnaFile::new(tmp_dna, tmp_wasm).await?;
    }
    Ok(dna)
}

#[async_trait::async_trait]
impl InterfaceApi for RealAdminInterfaceApi {
    type ApiRequest = AdminRequest;
    type ApiResponse = AdminResponse;

    async fn handle_request(
        &self,
        request: Result<Self::ApiRequest, SerializedBytesError>,
    ) -> InterfaceResult<Self::ApiResponse> {
        // Don't hold the read across both awaits
        {
            self.conductor_handle
                .check_running()
                .await
                .map_err(InterfaceError::RequestHandler)?;
        }
        match request {
            Ok(request) => Ok(AdminInterfaceApi::handle_admin_request(self, request).await),
            Err(e) => Ok(AdminResponse::Error(SerializationError::from(e).into())),
        }
    }
}

/// The Conductor lives inside an Arc<RwLock<_>> which is shared with all
/// other Api references
#[derive(Clone)]
pub struct RealAppInterfaceApi {
    conductor_handle: ConductorHandle,
}

impl RealAppInterfaceApi {
    /// Create a new instance from a shared Conductor reference
    pub fn new(conductor_handle: ConductorHandle) -> Self {
        Self { conductor_handle }
    }
}

#[async_trait::async_trait]
impl AppInterfaceApi for RealAppInterfaceApi {
    async fn invoke_zome(
        &self,
        _invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl InterfaceApi for RealAppInterfaceApi {
    type ApiRequest = AppRequest;
    type ApiResponse = AppResponse;
    async fn handle_request(
        &self,
        request: Result<Self::ApiRequest, SerializedBytesError>,
    ) -> InterfaceResult<Self::ApiResponse> {
        self.conductor_handle
            .check_running()
            .await
            .map_err(InterfaceError::RequestHandler)?;
        match request {
            Ok(request) => Ok(AppInterfaceApi::handle_request(self, request).await),
            Err(e) => Ok(AppResponse::Error {
                debug: e.to_string(),
            }),
        }
    }
}
/// Responses to requests received on an App interface
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum AppResponse {
    /// There has been an error in the request
    Error {
        // TODO maybe this could be serialized instead of stringified?
        /// Stringified version of the error
        debug: String,
    },
    /// The response to a zome call
    ZomeInvocationResponse {
        /// The data that was returned by this call
        response: Box<ZomeInvocationResponse>,
    },
}

/// Responses to messages received on an Admin interface
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum AdminResponse {
    /// This response is unimplemented
    Unimplemented(AdminRequest),
    /// [Dna] has successfully been installed
    DnaInstalled,
    /// AdminInterfaces have successfully been added
    AdminInterfacesAdded(()),
    /// A list of all installed [Dna]s
    ListDnas(Vec<DnaHash>),
    /// Keystore generated a new AgentPubKey
    GenerateAgentPubKey(AgentPubKey),
    /// Listing all the AgentPubKeys in the Keystore
    ListAgentPubKeys(Vec<AgentPubKey>),
    /// An error has ocurred in this request
    Error(ExternalApiWireError),
}

/// The set of messages that a conductor understands how to handle over an App interface
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum AppRequest {
    /// Asks the conductor to do some crypto
    CryptoRequest {
        /// The request payload
        request: Box<CryptoRequest>,
    },
    /// Call a zome function
    ZomeInvocationRequest {
        /// Information about which zome call you want to make
        request: Box<ZomeInvocation>,
    },
}

/// The set of messages that a conductor understands how to handle over an Admin interface
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum AdminRequest {
    /// Start a cell running
    Start(CellHandle),
    /// Stop a cell running
    Stop(CellHandle),
    /// Set up and register an Admin interface task
    AddAdminInterfaces(Vec<AdminInterfaceConfig>),
    /// Install a [Dna] from a path with optional properties
    InstallDna(PathBuf, Option<serde_json::Value>),
    /// List all installed [Dna]s
    ListDnas,
    /// Generate a new AgentPubKey
    GenerateAgentPubKey,
    /// List all AgentPubKeys in Keystore
    ListAgentPubKeys,
}

#[allow(missing_docs)]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum CryptoRequest {
    Sign(String),
    Decrypt(String),
    Encrypt(String),
}

#[allow(missing_docs)]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum TestRequest {
    AddAgent(AddAgentArgs),
}

#[allow(dead_code, missing_docs)]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AddAgentArgs {
    id: String,
    name: String,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::conductor::Conductor;
    use anyhow::Result;
    use holochain_types::test_utils::{fake_dna_file, write_fake_dna_file};
    use matches::assert_matches;
    use uuid::Uuid;

    #[tokio::test(threaded_scheduler)]
    async fn install_list_dna() -> Result<()> {
        let handle = Conductor::builder().test().await?.into_handle();
        let admin_api = RealAdminInterfaceApi::new(handle);
        let uuid = Uuid::new_v4();
        let dna = fake_dna_file(&uuid.to_string());
        let (dna_path, _tempdir) = write_fake_dna_file(dna.clone()).unwrap();
        let dna_hash = dna.dna_hash().clone();
        admin_api
            .handle_admin_request(AdminRequest::InstallDna(dna_path, None))
            .await;
        let dna_list = admin_api.handle_admin_request(AdminRequest::ListDnas).await;
        let expects = vec![dna_hash];
        assert_matches!(dna_list, AdminResponse::ListDnas(a) if a == expects);
        Ok(())
    }

    #[tokio::test(threaded_scheduler)]
    async fn generate_and_list_pub_keys() -> Result<()> {
        let handle = Conductor::builder().test().await?.into_handle();
        let admin_api = RealAdminInterfaceApi::new(handle);

        let agent_pub_key = admin_api
            .handle_admin_request(AdminRequest::GenerateAgentPubKey)
            .await;

        let agent_pub_key = match agent_pub_key {
            AdminResponse::GenerateAgentPubKey(key) => key,
            _ => panic!("bad type: {:?}", agent_pub_key),
        };

        let pub_key_list = admin_api
            .handle_admin_request(AdminRequest::ListAgentPubKeys)
            .await;

        let mut pub_key_list = match pub_key_list {
            AdminResponse::ListAgentPubKeys(list) => list,
            _ => panic!("bad type: {:?}", pub_key_list),
        };

        // includes our two pre-generated test keys
        let mut expects = vec![
            AgentPubKey::try_from("uhCAkw-zrttiYpdfAYX4fR6W8DPUdheZJ-1QsRA4cTImmzTYUcOr4").unwrap(),
            AgentPubKey::try_from("uhCAkomHzekU0-x7p62WmrusdxD2w9wcjdajC88688JGSTEo6cbEK").unwrap(),
            agent_pub_key,
        ];

        pub_key_list.sort();
        expects.sort();

        assert_eq!(expects, pub_key_list);

        Ok(())
    }

    #[tokio::test(threaded_scheduler)]
    async fn dna_read_parses() -> Result<()> {
        let uuid = Uuid::new_v4();
        let dna = fake_dna_file(&uuid.to_string());
        let (dna_path, _tmpdir) = write_fake_dna_file(dna.clone())?;
        let json = serde_json::json!({
            "test": "example",
            "how_many": 42,
        });
        let properties = Some(json.clone());
        let result = read_parse_dna(dna_path, properties).await?;
        let properties = Properties::new(json);
        let mut dna = dna.dna().clone();
        dna.properties = properties.try_into().unwrap();
        assert_eq!(&dna, result.dna());
        Ok(())
    }
}
