use crate::integrity::LinkTypes;
use hdk::prelude::*;
use holochain_test_wasm_common::*;

#[hdk_extern]
fn anchor(input: AnchorInput) -> ExternResult<EntryHash> {
    hdk::prelude::anchor(LinkTypes::Anchor, input.0, input.1)
}

#[hdk_extern]
fn anchor_many(inputs: ManyAnchorInput) -> ExternResult<Vec<EntryHash>> {
    let mut out = Vec::with_capacity(inputs.0.len());
    for input in inputs.0 {
        out.push(hdk::prelude::anchor(LinkTypes::Anchor, input.0, input.1)?);
    }
    Ok(out)
}

#[hdk_extern]
fn list_anchor_type_addresses(_: ()) -> ExternResult<Vec<AnyLinkableHash>> {
    hdk::prelude::list_anchor_type_addresses(LinkTypes::Anchor)
}

#[hdk_extern]
fn list_anchor_addresses(anchor_type: String) -> ExternResult<Vec<AnyLinkableHash>> {
    hdk::prelude::list_anchor_addresses(LinkTypes::Anchor, anchor_type)
}

#[hdk_extern]
fn list_anchor_tags(anchor_type: String) -> ExternResult<Vec<String>> {
    hdk::prelude::list_anchor_tags(LinkTypes::Anchor, anchor_type)
}
