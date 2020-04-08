//! capabilities implements the capability request functionality used to check
//! that a given capability has been granted for actions like zome calls

use crate::address::Address;
use crate::signature::{Provenance, Signature};
use holochain_serialized_bytes::prelude::*;

//--------------------------------------------------------------------------------------------------
// CapabilityRequest
//--------------------------------------------------------------------------------------------------

/// a struct to hold the capability information needed to make any capability request,
/// namely the provenance of the request (the agent address and signature) and the
/// actual token being used to make the request
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct CapabilityRequest {
    /// Address of capability token.
    pub cap_token: Address,

    /// Signature data for capability token.
    pub provenance: Provenance,
}

impl CapabilityRequest {
    /// Construct a new CapabilityRequest.
    pub fn new(token: Address, requester: Address, signature: Signature) -> Self {
        CapabilityRequest {
            cap_token: token,
            provenance: Provenance::new(requester, signature),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::address::Address;
    use sx_fixture::*;

    #[test]
    fn test_capability_request_new() {
        let cap_call = CapabilityRequest::new(
            Address::new("123".as_bytes().into()),
            Address::new("requester".as_bytes().into()),
            Signature::fixture(FixtureType::A),
        );
        assert_eq!(
            CapabilityRequest {
                cap_token: Address::new("123".as_bytes().into()),
                provenance: Provenance::new(Address::new("requester".as_bytes().into()), Signature::fixture(FixtureType::A)),
            },
            cap_call
        );
    }
}
