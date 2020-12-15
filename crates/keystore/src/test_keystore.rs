//! DANGER! This is a mock keystore for testing, DO NOT USE THIS IN PRODUCTION!

use crate::*;

const PUB1: &[u8] = &[
    154, 185, 40, 0, 115, 213, 127, 247, 174, 124, 110, 222, 11, 151, 230, 233, 2, 171, 91, 154,
    79, 50, 137, 45, 188, 110, 75, 56, 45, 18, 156, 158,
];
const SEC1: &[u8] = &[
    207, 84, 35, 155, 191, 10, 211, 240, 254, 92, 222, 153, 125, 241, 80, 102, 189, 217, 201, 140,
    112, 159, 21, 148, 138, 41, 85, 90, 169, 56, 174, 72,
];
const PUB2: &[u8] = &[
    123, 88, 252, 103, 102, 190, 254, 104, 167, 210, 29, 41, 26, 225, 12, 113, 137, 104, 253, 93,
    101, 214, 107, 125, 58, 208, 110, 203, 2, 166, 30, 88,
];
const SEC2: &[u8] = &[
    59, 31, 135, 117, 115, 107, 84, 52, 95, 216, 51, 180, 79, 81, 14, 169, 163, 149, 166, 174, 167,
    143, 3, 211, 123, 224, 24, 25, 201, 40, 81, 188,
];

/// Construct a new TestKeystore.
/// DANGER! This is a mock keystore for testing, DO NOT USE THIS IN PRODUCTION!
pub async fn spawn_test_keystore() -> KeystoreApiResult<KeystoreSender> {
    use lair_keystore_api::test::*;
    let (api, _evt) = spawn_test_keystore(
        vec![
            FixtureSignEd25519Keypair {
                pub_key: PUB1.to_vec(),
                priv_key: SEC1.to_vec(),
            },
            FixtureSignEd25519Keypair {
                pub_key: PUB2.to_vec(),
                priv_key: SEC2.to_vec(),
            },
        ],
        vec![],
        vec![],
    )
    .await?;
    Ok(api)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_test_keystore() {
        tokio::task::spawn(async move {
            let keystore = spawn_test_keystore().await.unwrap();
            let agent_pubkey1 = holo_hash::AgentPubKey::new_from_pure_entropy(&keystore)
                .await
                .unwrap();
            assert_eq!(
                "uhCAkmrkoAHPVf_eufG7eC5fm6QKrW5pPMoktvG5LOC0SnJ4vV1Uv",
                &agent_pubkey1.to_string()
            );
            let agent_pubkey2 = holo_hash::AgentPubKey::new_from_pure_entropy(&keystore)
                .await
                .unwrap();
            assert_eq!(
                "uhCAke1j8Z2a-_min0h0pGuEMcYlo_V1l1mt9OtBuywKmHlg4L_R-",
                &agent_pubkey2.to_string()
            );

            #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
            struct MyData(Vec<u8>);

            let my_data_1 = MyData(b"signature test data 1".to_vec());

            let signature = agent_pubkey1.sign(&keystore, &my_data_1).await.unwrap();

            assert!(agent_pubkey1
                .verify_signature(&signature, &my_data_1)
                .await
                .unwrap());
        })
        .await
        .unwrap();
    }
}
