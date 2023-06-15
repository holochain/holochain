use hdk::prelude::*;

#[hdk_extern]
fn create_cap_grant_for_private_function(_: ()) -> ExternResult<CapSecret> {
    let cap_secret = generate_cap_secret()?;

    let mut functions = BTreeSet::new();
    functions.insert((zome_info()?.name, "private_function".into()));

    // If you comment out this call, the test behaves as expected.
    create_cap_grant(CapGrantEntry {
        tag: "".into(),
        access: cap_secret.into(),
        functions: GrantedFunctions::Listed(functions),
    })?;

    Ok(cap_secret)
}

#[hdk_extern]
fn private_function(_: ()) -> ExternResult<String> {
    Ok("this is the result of the private function".to_string())
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
pub struct RemoteCallPrivateInput {
    pub to_cell: CellId,
    pub maybe_cap_secret: Option<CapSecret>,
}

#[hdk_extern]
fn remote_call_private_function(input: RemoteCallPrivateInput) -> ExternResult<String> {
    let zome_name = zome_info()?.name;

    let RemoteCallPrivateInput {
        to_cell,
        maybe_cap_secret,
    } = input;

    match hdk::p2p::call_remote(
        to_cell.agent_pubkey().clone(),
        zome_name,
        FunctionName::new("private_function".to_owned()),
        maybe_cap_secret,
        Some(()),
    )? {
        ZomeCallResponse::Ok(response) => match response.decode() {
            Ok(r) => Ok(r),
            Err(e) => Err(wasm_error!(WasmErrorInner::Guest(e.to_string()))),
        },
        ZomeCallResponse::Unauthorized(..) => Err(wasm_error!(WasmErrorInner::CallError(
            "Unauthorized call to private_function".to_string()
        ))),
        ZomeCallResponse::NetworkError(_) => Err(wasm_error!(WasmErrorInner::CallError(
            "Network error while calling private_function".to_string()
        ))),
        ZomeCallResponse::CountersigningSession(_) => Err(wasm_error!(WasmErrorInner::CallError(
            "Unexpected CountersigningSession while calling private_function".to_string()
        ))),
    }
}
