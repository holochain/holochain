use crate::prelude::*;

pub mod short_hand;

pub fn set_zome_types(entries: &[(u8, u8)], links: &[(u8, u8)]) {
    struct TestHdi(ScopedZomeTypesSet);
    #[allow(unused_variables)]
    impl HdiT for TestHdi {
        fn verify_signature(&self, verify_signature: VerifySignature) -> ExternResult<bool> {
            todo!()
        }

        fn hash(&self, hash_input: HashInput) -> ExternResult<HashOutput> {
            todo!()
        }

        fn must_get_entry(
            &self,
            must_get_entry_input: MustGetEntryInput,
        ) -> ExternResult<EntryHashed> {
            todo!()
        }

        fn must_get_action(
            &self,
            must_get_action_input: MustGetActionInput,
        ) -> ExternResult<SignedActionHashed> {
            todo!()
        }

        fn must_get_valid_record(
            &self,
            must_get_valid_record_input: MustGetValidRecordInput,
        ) -> ExternResult<Record> {
            todo!()
        }

        fn dna_info(&self, dna_info_input: ()) -> ExternResult<DnaInfo> {
            todo!()
        }

        fn zome_info(&self, zome_info_input: ()) -> ExternResult<ZomeInfo> {
            let info = ZomeInfo {
                name: String::default().into(),
                id: u8::default().into(),
                properties: Default::default(),
                entry_defs: EntryDefs(Default::default()),
                extern_fns: Default::default(),
                zome_types: self.0.clone(),
            };
            Ok(info)
        }

        fn x_salsa20_poly1305_decrypt(
            &self,
            x_salsa20_poly1305_decrypt: XSalsa20Poly1305Decrypt,
        ) -> ExternResult<Option<XSalsa20Poly1305Data>> {
            todo!()
        }

        fn x_25519_x_salsa20_poly1305_decrypt(
            &self,
            x_25519_x_salsa20_poly1305_decrypt: X25519XSalsa20Poly1305Decrypt,
        ) -> ExternResult<Option<XSalsa20Poly1305Data>> {
            todo!()
        }

        fn trace(&self, trace_msg: TraceMsg) -> ExternResult<()> {
            todo!()
        }

        fn must_get_agent_activity(
            &self,
            must_get_agent_activity_input: MustGetAgentActivityInput,
        ) -> ExternResult<Vec<RegisterAgentActivity>> {
            todo!()
        }
    }
    set_hdi(TestHdi(ScopedZomeTypesSet {
        entries: ScopedZomeTypes(
            entries
                .into_iter()
                .map(|(z, types)| (ZomeId(*z), (0..*types).map(|t| EntryDefIndex(t)).collect()))
                .collect(),
        ),
        links: ScopedZomeTypes(
            links
                .into_iter()
                .map(|(z, types)| (ZomeId(*z), (0..*types).map(|t| LinkType(t)).collect()))
                .collect(),
        ),
    }));
}
