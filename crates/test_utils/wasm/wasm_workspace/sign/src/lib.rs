use hdk3::prelude::*;

#[hdk_extern]
fn sign(sign_input: Sign) -> ExternResult<Signature> {
    Ok(hdk3::prelude::sign_raw(sign_input.key, sign_input.data)?)
}

#[hdk_extern]
fn verify_signature_raw(
    verify_signature_input: VerifySignature,
) -> ExternResult<bool> {
    let VerifySignature {
        key,
        signature,
        data,
    } = verify_signature_input;
    hdk3::prelude::verify_signature_raw(
        key, signature, data,
    )
}

#[derive(serde::Serialize, std::fmt::Debug, Clone)]
struct SomeStruct {
    foo: String,
    bar: u32,
}

#[hdk_extern]
fn verify_signature(
    agent_pub_key: AgentPubKey,
) -> ExternResult<()> {

    let some_struct = SomeStruct{
        foo: String::from("Foo"),
        bar: 100,
    };

    let signature = match hdk3::prelude::sign(agent_pub_key.clone(), some_struct.clone()) {
        Ok(v) => v,
        Err(error) => {
            tracing::error!(?agent_pub_key, ?some_struct, ?error);
            return Err(error);
        },
    };

    tracing::debug!(?signature);

    let verify = match hdk3::prelude::verify_signature(agent_pub_key.clone(), signature.clone(), some_struct.clone()) {
        Ok(v) => v,
        Err(error) => {
            tracing::error!(?agent_pub_key, ?some_struct, ?signature, ?error);
            return Err(error);
        }
    };

    assert!(verify);

    let bad_struct = SomeStruct{
        foo: String::from("foo"),
        bar: 100,
    };

    let not_verify = match hdk3::prelude::verify_signature(agent_pub_key.clone(), signature.clone(), bad_struct.clone()) {
        Ok(v) => v,
        Err(error) => {
            tracing::error!(?agent_pub_key, ?bad_struct, ?signature, ?error);
            return Err(error);
        }
    };

    assert!(!not_verify);

    Ok(())
}
