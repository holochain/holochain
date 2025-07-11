//! # Holochain Mock HDI
//!
//! This is a simple utility crate that allows mocking the HDI.
//!
//! # Examples
//!
//! ```
//! use hdi::prelude::*;
//!
//! // Create the mock.
//! let mock_hdi = holochain_mock_hdi::MockHdiT::new();
//!
//! // Create the a return type.
//! let empty_agent_key = AgentPubKey::from_raw_36(vec![0u8; 36]);
//!
//! // Set the HDI to use the mock.
//! set_hdi(mock_hdi);
//!
//! // Call the HDI and the mock will run.
//! let dna_info = HDI.with(|i| i.borrow().dna_info(())).unwrap();
//! ```

use hdi::prelude::*;

::mockall::mock! {
    pub HdiT {}
    impl HdiT for HdiT {
        fn verify_signature(&self, verify_signature: VerifySignature) -> ExternResult<bool>;
        fn must_get_entry(&self, must_get_entry_input: MustGetEntryInput) -> ExternResult<EntryHashed>;
        fn must_get_action(
            &self,
            must_get_action_input: MustGetActionInput,
        ) -> ExternResult<SignedActionHashed>;
        fn must_get_valid_record(
            &self,
            must_get_valid_record_input: MustGetValidRecordInput,
        ) -> ExternResult<Record>;
        fn must_get_agent_activity(
            &self,
            input: MustGetAgentActivityInput,
        ) -> ExternResult<Vec<RegisterAgentActivity>>;
        // Info
        fn dna_info(&self, dna_info_input: ()) -> ExternResult<DnaInfo>;
        fn zome_info(&self, zome_info_input: ()) -> ExternResult<ZomeInfo>;
        // Trace
        fn trace(&self, trace_msg: TraceMsg) -> ExternResult<()>;
        // XSalsa20Poly1305
        fn x_salsa20_poly1305_decrypt(
            &self,
            x_salsa20_poly1305_decrypt: XSalsa20Poly1305Decrypt,
        ) -> ExternResult<Option<XSalsa20Poly1305Data>>;
        fn x_25519_x_salsa20_poly1305_decrypt(
            &self,
            x_25519_x_salsa20_poly1305_decrypt: X25519XSalsa20Poly1305Decrypt,
        ) -> ExternResult<Option<XSalsa20Poly1305Data>>;
        fn ed_25519_x_salsa20_poly1305_decrypt(
            &self,
            ed_25519_x_salsa20_poly1305_decrypt: Ed25519XSalsa20Poly1305Decrypt,
        ) -> ExternResult<XSalsa20Poly1305Data>;
    }

}
