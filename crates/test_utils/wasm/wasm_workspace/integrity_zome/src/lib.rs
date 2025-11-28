use hdi::prelude::*;

#[hdk_entry_helper]
pub struct Post(pub String);
#[hdk_entry_helper]
pub struct Msg(pub String);

#[hdk_entry_helper]
pub struct PrivMsg(pub String);

#[hdk_entry_types]
#[unit_enum(UnitEntryTypes)]
pub enum EntryTypes {
    #[entry_type(required_validations = 5)]
    Post(Post), // "post"
    #[entry_type(required_validations = 5)]
    Msg(Msg),
    #[entry_type(required_validations = 5, visibility = "private")]
    PrivMsg(PrivMsg),
}

#[hdk_extern]
fn genesis_self_check(data: GenesisSelfCheckData) -> ExternResult<ValidateCallbackResult> {
    let GenesisSelfCheckDataV2 {
        membrane_proof: _maybe_membrane_proof,
        agent_key: _agent_key,
    } = data;
    Ok(ValidateCallbackResult::Valid)
}

#[hdk_extern]
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    if let Op::StoreEntry(StoreEntry {
        action:
            SignedHashed {
                hashed: HoloHashed {
                    content: action, ..
                },
                ..
            },
        entry,
    }) = op
    {
        if let Some(AppEntryDef {
            entry_index: entry_def_index,
            zome_index,
            ..
        }) = action.app_entry_def()
        {
            match EntryTypes::deserialize_from_type(*zome_index, *entry_def_index, &entry)? {
                Some(EntryTypes::Post(_)) => (),
                Some(EntryTypes::Msg(_)) => (),
                Some(EntryTypes::PrivMsg(_)) => (),
                None => (),
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
fn call_must_get_entry(must_get_entry_input: MustGetEntryInput) -> ExternResult<EntryHashed> {
    HDI.with(|i| i.borrow().must_get_entry(must_get_entry_input))
}
#[hdk_extern]
fn call_must_get_action(
    must_get_action_input: MustGetActionInput,
) -> ExternResult<SignedActionHashed> {
    HDI.with(|i| i.borrow().must_get_action(must_get_action_input))
}
#[hdk_extern]
fn call_must_get_valid_record(
    must_get_valid_record_input: MustGetValidRecordInput,
) -> ExternResult<Record> {
    HDI.with(|i| {
        i.borrow()
            .must_get_valid_record(must_get_valid_record_input)
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
