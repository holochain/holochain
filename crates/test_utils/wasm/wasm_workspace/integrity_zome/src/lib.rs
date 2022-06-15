use holochain_deterministic_integrity::prelude::*;

#[hdk_entry_helper]
pub struct Post(pub String);
#[hdk_entry_helper]
pub struct Msg(pub String);

#[hdk_entry_helper]
pub struct PrivMsg(pub String);

#[hdk_entry_defs]
#[unit_enum(UnitEntryTypes)]
pub enum EntryTypes {
    #[entry_def(required_validations = 5)]
    Post(Post), // "post"
    #[entry_def(required_validations = 5)]
    Msg(Msg),
    #[entry_def(required_validations = 5, visibility = "private")]
    PrivMsg(PrivMsg),
}

#[hdk_extern]
fn genesis_self_check(data: GenesisSelfCheckData) -> ExternResult<ValidateCallbackResult> {
    let GenesisSelfCheckData {
        dna_info: _,
        membrane_proof: _,
        agent_key: _,
    } = data;
    Ok(ValidateCallbackResult::Valid)
}

#[hdk_extern]
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    if let Op::StoreEntry {
        header:
            SignedHashed {
                hashed: HoloHashed {
                    content: header, ..
                },
                ..
            },
        entry,
    } = op
    {
        if let Some(AppEntryType {
            id: entry_def_index,
            ..
        }) = header.app_entry_type()
        {
            match zome_info()?
                .zome_types
                .entries
                .to_local_scope(*entry_def_index)
            {
                Some(local_type_index) => {
                    match EntryTypes::try_from_local_type(local_type_index, &entry) {
                        EntryCheck::Found(ParseEntry::Valid(EntryTypes::Post(_))) => (),
                        EntryCheck::Found(ParseEntry::Valid(EntryTypes::Msg(_))) => (),
                        EntryCheck::Found(ParseEntry::Valid(EntryTypes::PrivMsg(_))) => (),
                        EntryCheck::Found(ParseEntry::Failed(_)) => (),
                        EntryCheck::NotInScope => (),
                    }
                }
                None => (),
            }
            match zome_info()?
                .zome_types
                .entries
                .to_local_scope(*entry_def_index)
            {
                Some(local_index) => match local_index.try_into() {
                    Ok(UnitEntryTypes::Post) => (),
                    _ => (),
                },
                None => (),
            }
            match EntryTypes::try_from_global_type(*entry_def_index, &entry)? {
                EntryCheck::Found(ParseEntry::Valid(EntryTypes::Post(_))) => (),
                EntryCheck::Found(ParseEntry::Valid(EntryTypes::Msg(_))) => (),
                EntryCheck::Found(ParseEntry::Valid(EntryTypes::PrivMsg(_))) => (),
                EntryCheck::Found(ParseEntry::Failed(_)) => (),
                EntryCheck::NotInScope => (),
            }
            match EntryTypes::try_from_local_type(UnitEntryTypes::Post, &entry) {
                EntryCheck::Found(ParseEntry::Valid(EntryTypes::Post(_))) => (),
                EntryCheck::Found(ParseEntry::Valid(EntryTypes::Msg(_))) => (),
                EntryCheck::Found(ParseEntry::Valid(EntryTypes::PrivMsg(_))) => (),
                EntryCheck::Found(ParseEntry::Failed(_)) => (),
                EntryCheck::NotInScope => (),
            }
        }
    }
    Ok(ValidateCallbackResult::Valid)
}

#[hdk_extern]
fn call_verify_signature(verify_signature: VerifySignature) -> ExternResult<bool> {
    HDI.with(|i| i.borrow().verify_signature(verify_signature))
}
#[hdk_extern]
fn call_hash(hash_input: HashInput) -> ExternResult<HashOutput> {
    HDI.with(|i| i.borrow().hash(hash_input))
}
#[hdk_extern]
fn call_must_get_entry(must_get_entry_input: MustGetEntryInput) -> ExternResult<EntryHashed> {
    HDI.with(|i| i.borrow().must_get_entry(must_get_entry_input))
}
#[hdk_extern]
fn call_must_get_header(
    must_get_header_input: MustGetHeaderInput,
) -> ExternResult<SignedHeaderHashed> {
    HDI.with(|i| i.borrow().must_get_header(must_get_header_input))
}
#[hdk_extern]
fn call_must_get_valid_element(
    must_get_valid_element_input: MustGetValidElementInput,
) -> ExternResult<Element> {
    HDI.with(|i| {
        i.borrow()
            .must_get_valid_element(must_get_valid_element_input)
    })
}
// Info
#[hdk_extern]
fn call_dna_info(dna_info_input: ()) -> ExternResult<DnaInfo> {
    HDI.with(|i| i.borrow().dna_info(dna_info_input))
}
#[hdk_extern]
fn call_zome_info(zome_info_input: ()) -> ExternResult<ZomeInfo> {
    HDI.with(|i| i.borrow().zome_info(zome_info_input))
}
// Trace
#[hdk_extern]
fn call_trace(trace_msg: TraceMsg) -> ExternResult<()> {
    HDI.with(|i| i.borrow().trace(trace_msg))
}
// XSalsa20Poly1305
#[hdk_extern]
fn call_x_salsa20_poly1305_decrypt(
    x_salsa20_poly1305_decrypt: XSalsa20Poly1305Decrypt,
) -> ExternResult<Option<XSalsa20Poly1305Data>> {
    HDI.with(|i| {
        i.borrow()
            .x_salsa20_poly1305_decrypt(x_salsa20_poly1305_decrypt)
    })
}
#[hdk_extern]
fn call_x_25519_x_salsa20_poly1305_decrypt(
    x_25519_x_salsa20_poly1305_decrypt: X25519XSalsa20Poly1305Decrypt,
) -> ExternResult<Option<XSalsa20Poly1305Data>> {
    HDI.with(|i| {
        i.borrow()
            .x_25519_x_salsa20_poly1305_decrypt(x_25519_x_salsa20_poly1305_decrypt)
    })
}

#[cfg(all(test, feature = "mock"))]
pub mod test {
    use holochain_deterministic_integrity::prelude::holo_hash::DnaHash;

    use super::*;
    #[test]
    fn test_all_holochain_deterministic_integrity() {
        let mut mock_hdi = holochain_mock_hdi::MockHdiT::new();
        let empty_agent_key = AgentPubKey::from_raw_36(vec![0u8; 36]);
        let empty_header_hash = HeaderHash::from_raw_36(vec![0u8; 36]);
        let empty_dna_hash = DnaHash::from_raw_36(vec![0u8; 36]);

        mock_hdi
            .expect_verify_signature()
            .once()
            .returning(|_| Ok(true));

        mock_hdi.expect_hash().once().returning({
            let empty_agent_key = empty_agent_key.clone();
            move |_| Ok(HashOutput::Entry(empty_agent_key.clone().into()))
        });

        mock_hdi.expect_must_get_entry().once().returning({
            let empty_agent_key = empty_agent_key.clone();
            move |_| {
                Ok(EntryHashed::with_pre_hashed(
                    Entry::Agent(empty_agent_key.clone()),
                    empty_agent_key.clone().into(),
                ))
            }
        });

        let dna = SignedHeaderHashed::with_presigned(
            HeaderHashed::with_pre_hashed(
                Header::Dna(Dna {
                    author: empty_agent_key.clone(),
                    timestamp: Timestamp::from_micros(0),
                    hash: empty_dna_hash.clone(),
                }),
                empty_header_hash.clone(),
            ),
            Signature([0u8; 64]),
        );

        mock_hdi.expect_must_get_header().once().returning({
            let dna = dna.clone();
            move |_| Ok(dna.clone())
        });

        mock_hdi.expect_must_get_valid_element().once().returning({
            let dna = dna.clone();
            move |_| Ok(Element::new(dna.clone(), None))
        });

        mock_hdi.expect_dna_info().once().returning({
            let empty_dna_hash = empty_dna_hash.clone();
            move |_| {
                Ok(DnaInfo {
                    name: "".to_string(),
                    hash: empty_dna_hash.clone(),
                    properties: UnsafeBytes::from(vec![]).into(),
                    zome_names: vec![],
                })
            }
        });

        mock_hdi.expect_zome_info().once().returning({
            move |_| {
                Ok(ZomeInfo {
                    name: "".to_string().into(),
                    properties: UnsafeBytes::from(vec![]).into(),
                    id: 0.into(),
                    entry_defs: EntryDefs(vec![]),
                    extern_fns: vec![],
                    zome_types: Default::default(),
                })
            }
        });

        set_hdi(mock_hdi);

        call_verify_signature(VerifySignature {
            key: empty_agent_key.clone(),
            signature: Signature([0u8; 64]),
            data: vec![],
        })
        .unwrap();

        call_hash(HashInput::Entry(Entry::Agent(empty_agent_key.clone()))).unwrap();

        call_must_get_entry(MustGetEntryInput(empty_agent_key.clone().into())).unwrap();

        call_must_get_header(MustGetHeaderInput(empty_header_hash.clone())).unwrap();

        call_must_get_valid_element(MustGetValidElementInput(empty_header_hash.clone())).unwrap();

        call_dna_info(()).unwrap();

        call_zome_info(()).unwrap();
    }
}
