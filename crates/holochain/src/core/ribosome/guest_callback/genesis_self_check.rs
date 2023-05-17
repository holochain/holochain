pub mod v1;
pub mod v2;

use holochain_zome_types::GenesisSelfCheckDataV1;
use crate::core::ribosome::guest_callback::genesis_self_check::v1::GenesisSelfCheckInvocationV1;
use crate::core::ribosome::guest_callback::genesis_self_check::v2::GenesisSelfCheckInvocationV2;
use holochain_zome_types::GenesisSelfCheckDataV2;
use crate::core::ribosome::guest_callback::genesis_self_check::v1::GenesisSelfCheckHostAccessV1;
use crate::core::ribosome::guest_callback::genesis_self_check::v2::GenesisSelfCheckHostAccessV2;
use holochain_types::prelude::HostFnAccess;
use std::sync::Arc;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;
use derive_more::Constructor;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum GenesisSelfCheckResult {
    Valid,
    Invalid(String),
}

#[derive(Clone, Constructor, Debug)]
pub struct GenesisSelfCheckHostAccess {
    pub host_access_1: GenesisSelfCheckHostAccessV1,
    pub host_access_2: GenesisSelfCheckHostAccessV2,
}

impl From<&GenesisSelfCheckHostAccess> for HostFnAccess {
    fn from(_: &GenesisSelfCheckHostAccess) -> Self {
        let mut access = Self::none();
        access.keystore_deterministic = Permission::Allow;
        access.bindings_deterministic = Permission::Allow;
        access
    }
}

#[derive(Clone)]
pub struct GenesisSelfCheckInvocation {
    pub data_1: Arc<GenesisSelfCheckDataV1>,
    pub data_2: Arc<GenesisSelfCheckDataV2>,
}

impl From<GenesisSelfCheckInvocation> for (GenesisSelfCheckInvocationV1, GenesisSelfCheckInvocationV2) {
    fn from(invocation: GenesisSelfCheckInvocation) -> Self {
        (
            GenesisSelfCheckInvocationV1 { payload: invocation.data_1 },
            GenesisSelfCheckInvocationV2 { payload: invocation.data_2 }
        )
    }
}