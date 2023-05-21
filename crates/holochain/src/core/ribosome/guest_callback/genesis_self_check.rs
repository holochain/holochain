pub mod v1;
pub mod v2;

use crate::core::ribosome::guest_callback::genesis_self_check::v1::GenesisSelfCheckHostAccessV1;
use crate::core::ribosome::guest_callback::genesis_self_check::v1::GenesisSelfCheckInvocationV1;
use crate::core::ribosome::guest_callback::genesis_self_check::v2::GenesisSelfCheckHostAccessV2;
use crate::core::ribosome::guest_callback::genesis_self_check::v2::GenesisSelfCheckInvocationV2;
use crate::core::ribosome::guest_callback::genesis_self_check::v1::GenesisSelfCheckResultV1;
use crate::core::ribosome::guest_callback::genesis_self_check::v2::GenesisSelfCheckResultV2;
use derive_more::Constructor;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::GenesisSelfCheckDataV1;
use holochain_zome_types::GenesisSelfCheckDataV2;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum GenesisSelfCheckResult {
    Valid,
    Invalid(String),
}

impl From<GenesisSelfCheckResultV1> for GenesisSelfCheckResult {
    fn from(result_v1: GenesisSelfCheckResultV1) -> Self {
        match result_v1 {
            GenesisSelfCheckResultV1::Valid => Self::Valid,
            GenesisSelfCheckResultV1::Invalid(s) => Self::Invalid(s),
        }
    }
}

impl From<GenesisSelfCheckResultV2> for GenesisSelfCheckResult {
    fn from(result_v2: GenesisSelfCheckResultV2) -> Self {
        match result_v2 {
            GenesisSelfCheckResultV2::Valid => Self::Valid,
            GenesisSelfCheckResultV2::Invalid(s) => Self::Invalid(s),
        }
    }
}

#[derive(Clone, Constructor, Debug)]
pub struct GenesisSelfCheckHostAccess {
    pub host_access_1: GenesisSelfCheckHostAccessV1,
    pub host_access_2: GenesisSelfCheckHostAccessV2,
}

impl From<GenesisSelfCheckHostAccess>
    for (GenesisSelfCheckHostAccessV1, GenesisSelfCheckHostAccessV2)
{
    fn from(invocation: GenesisSelfCheckHostAccess) -> Self {
        (invocation.host_access_1, invocation.host_access_2)
    }
}

#[derive(Clone)]
pub struct GenesisSelfCheckInvocation {
    pub data_1: Arc<GenesisSelfCheckDataV1>,
    pub data_2: Arc<GenesisSelfCheckDataV2>,
}

impl From<GenesisSelfCheckInvocation>
    for (GenesisSelfCheckInvocationV1, GenesisSelfCheckInvocationV2)
{
    fn from(invocation: GenesisSelfCheckInvocation) -> Self {
        (
            GenesisSelfCheckInvocationV1 {
                payload: invocation.data_1,
            },
            GenesisSelfCheckInvocationV2 {
                payload: invocation.data_2,
            },
        )
    }
}
