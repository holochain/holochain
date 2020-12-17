//! the _host_ types used to track the status/result of validating entries
//! c.f. _guest_ types for validation callbacks and packages across the wasm boudary in zome_types

use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::prelude::*;

#[derive(
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    SerializedBytes,
    derive_more::From,
    derive_more::Into,
)]
/// Type for sending responses to `get_validation_package`
pub struct ValidationPackageResponse(pub Option<ValidationPackage>);
