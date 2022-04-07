//! # Holochain Mock IDK
//!
//! This is a simple utility crate that allows mocking the IDK.
//!
//! # Examples
//!
//! ```
//! use idk::prelude::*;
//!
//! // Create the mock.
//! let mut mock_idk = holochain_mock_idk::MockIdkT::new();
//!
//! // Create the a return type.
//! let empty_agent_key = AgentPubKey::from_raw_36(vec![0u8; 36]);
//!
//! // Setup the expectation.
//! mock_idk.expect_hash().once().returning({
//!     let empty_agent_key = empty_agent_key.clone();
//!     move |_| Ok(HashOutput::Entry(empty_agent_key.clone().into()))
//! });
//!
//! // Set the IDK to use the mock.
//! set_idk(mock_idk);
//!
//! // Create an input type.
//! let hash_input = HashInput::Entry(Entry::Agent(empty_agent_key.clone()));
//!
//! // Call the IDK and the mock will run.
//! let hash_output = IDK.with(|i| i.borrow().hash(hash_input)).unwrap();
//!
//! assert!(matches!(
//!     hash_output,
//!     HashOutput::Entry(output) if output == EntryHash::from(empty_agent_key)
//! ));
//! ```

use idk::prelude::*;

::mockall::mock! {
    pub IdkT {}
    impl IdkT for IdkT {
        fn verify_signature(&self, verify_signature: VerifySignature) -> ExternResult<bool>;
        fn hash(&self, hash_input: HashInput) -> ExternResult<HashOutput>;
        fn must_get_entry(&self, must_get_entry_input: MustGetEntryInput) -> ExternResult<EntryHashed>;
        fn must_get_header(
            &self,
            must_get_header_input: MustGetHeaderInput,
        ) -> ExternResult<SignedHeaderHashed>;
        fn must_get_valid_element(
            &self,
            must_get_valid_element_input: MustGetValidElementInput,
        ) -> ExternResult<Element>;
        // Info
        fn dna_info(&self, dna_info_input: ()) -> ExternResult<DnaInfo>;
        fn zome_info(&self, zome_info_input: ()) -> ExternResult<ZomeInfo>;
        // Trace
        #[cfg(feature = "trace")]
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
