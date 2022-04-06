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
