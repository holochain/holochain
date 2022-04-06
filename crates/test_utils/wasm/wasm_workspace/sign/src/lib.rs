use hdk::prelude::*;

#[hdk_extern]
fn sign(sign_input: Sign) -> ExternResult<Signature> {
    hdk::prelude::sign_raw(sign_input.key, sign_input.data.to_vec())
}

#[hdk_extern]
fn sign_ephemeral(_: ()) -> ExternResult<Vec<EphemeralSignatures>> {
    #[derive(Serialize, Deserialize, Debug)]
    struct One([u8; 2]);
    #[derive(Serialize, Deserialize, Debug)]
    struct Two([u8; 2]);
    Ok(vec![
        // Can use normal sign_ephemeral if all the types are the same.
        hdk::prelude::sign_ephemeral(vec![One([1, 2]), One([3, 4])])?,
        // Need to use raw if the types are different.
        hdk::prelude::sign_ephemeral_raw(
            vec![
                holochain_serialized_bytes::encode(&One([1, 2])).map_err(|e| wasm_error!(e.into()))?,
                holochain_serialized_bytes::encode(&Two([2, 3])).map_err(|e| wasm_error!(e.into()))?,
            ]
        )?
    ])
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
    hdk::prelude::verify_signature_raw(
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

    let signature = match hdk::prelude::sign(agent_pub_key.clone(), some_struct.clone()) {
        Ok(v) => v,
        Err(error) => {
            tracing::error!(?agent_pub_key, ?some_struct, ?error);
            return Err(error);
        },
    };

    tracing::debug!(?signature);

    let verify = match hdk::prelude::verify_signature(agent_pub_key.clone(), signature.clone(), some_struct.clone()) {
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

    let not_verify = match hdk::prelude::verify_signature(agent_pub_key.clone(), signature.clone(), bad_struct.clone()) {
        Ok(v) => v,
        Err(error) => {
            tracing::error!(?agent_pub_key, ?bad_struct, ?signature, ?error);
            return Err(error);
        }
    };

    assert!(!not_verify);

    Ok(())
}

#[cfg(test)]
pub mod tests {
    use hdk::prelude::*;
    use ::fixt::prelude::{fixt, Predictable};

    #[test]
    fn sign_ephemeral_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let pubkey = fixt!(AgentPubKey);
        let signatures: Vec<Signature> = SignatureFixturator::new(Predictable).take(2).collect();

        mock_hdk.expect_sign_ephemeral()
            .times(2)
            .return_const(Ok(EphemeralSignatures {
                key: pubkey.clone(),
                signatures: signatures.clone(),
            }));

        hdk::prelude::set_hdk(mock_hdk);

        let output = super::sign_ephemeral(()).unwrap();

        assert_eq!(
            output,
            vec![
                EphemeralSignatures {
                    key: pubkey.clone(),
                    signatures: signatures.clone(),
                },
                EphemeralSignatures {
                    key: pubkey.clone(),
                    signatures: signatures.clone(),
                }
            ]
        )
    }
}