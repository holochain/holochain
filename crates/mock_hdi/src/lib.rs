//! # Holochain Mock HDI
//!
//! This is a simple utility crate that allows mocking the HDI.
//!
//! # Examples
//!
//! ```
//! use holochain_deterministic_integrity::prelude::*;
//!
//! // Create the mock.
//! let mut mock_hdi = holochain_mock_hdi::MockHdiT::new();
//!
//! // Create the a return type.
//! let empty_agent_key = AgentPubKey::from_raw_36(vec![0u8; 36]);
//!
//! // Setup the expectation.
//! mock_hdi.expect_hash().once().returning({
//!     let empty_agent_key = empty_agent_key.clone();
//!     move |_| Ok(HashOutput::Entry(empty_agent_key.clone().into()))
//! });
//!
//! // Set the HDI to use the mock.
//! set_hdi(mock_hdi);
//!
//! // Create an input type.
//! let hash_input = HashInput::Entry(Entry::Agent(empty_agent_key.clone()));
//!
//! // Call the HDI and the mock will run.
//! let hash_output = HDI.with(|i| i.borrow().hash(hash_input)).unwrap();
//!
//! assert!(matches!(
//!     hash_output,
//!     HashOutput::Entry(output) if output == EntryHash::from(empty_agent_key)
//! ));
//! ```

use holochain_deterministic_integrity::prelude::*;

::mockall::mock! {
    pub HdiT {}
    impl HdiT for HdiT {
        fn verify_signature(&self, verify_signature: VerifySignature) -> ExternResult<bool>;
        fn hash(&self, hash_input: HashInput) -> ExternResult<HashOutput>;
        fn must_get_entry(&self, must_get_entry_input: MustGetEntryInput) -> ExternResult<EntryHashed>;
        fn must_get_action(
            &self,
            must_get_action_input: MustGetActionInput,
        ) -> ExternResult<SignedActionHashed>;
        fn must_get_valid_element(
            &self,
            must_get_valid_element_input: MustGetValidElementInput,
        ) -> ExternResult<Element>;
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
    }

}
