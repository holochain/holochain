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

const X25519_SEC1: [u8; 32] = [
    253, 12, 117, 61, 12, 47, 207, 107, 110, 116, 6, 194, 214, 88, 61, 161, 220, 6, 53, 190, 225,
    254, 230, 143, 130, 70, 25, 160, 15, 168, 42, 37,
];
const X25519_PUB1: [u8; 32] = [
    65, 17, 71, 31, 48, 10, 48, 208, 3, 220, 71, 246, 83, 246, 74, 221, 3, 123, 54, 48, 160, 192,
    179, 207, 115, 6, 19, 53, 233, 231, 167, 75,
];

const X25519_SEC2: [u8; 32] = [
    19, 195, 209, 22, 152, 172, 136, 179, 66, 40, 251, 5, 43, 170, 48, 164, 199, 79, 46, 241, 70,
    51, 70, 218, 21, 43, 220, 65, 117, 102, 224, 133,
];
const X25519_PUB2: [u8; 32] = [
    139, 250, 5, 51, 172, 9, 244, 251, 44, 226, 178, 145, 1, 252, 128, 237, 27, 225, 11, 171, 153,
    205, 115, 228, 72, 211, 110, 41, 115, 48, 251, 98,
];

const X25519_SEC3: [u8; 32] = [
    229, 85, 118, 86, 0, 47, 249, 160, 87, 152, 212, 133, 41, 244, 102, 240, 175, 147, 71, 212,
    107, 100, 148, 173, 27, 189, 83, 63, 162, 97, 248, 133,
];
const X25519_PUB3: [u8; 32] = [
    211, 158, 23, 148, 162, 67, 112, 72, 185, 58, 136, 103, 76, 164, 39, 200, 83, 124, 57, 64, 234,
    36, 102, 209, 80, 32, 77, 68, 108, 242, 71, 41,
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
        vec![
            FixtureX25519Keypair {
                pub_key: X25519_PUB1.into(),
                priv_key: X25519_SEC1.into(),
            },
            FixtureX25519Keypair {
                pub_key: X25519_PUB2.into(),
                priv_key: X25519_SEC2.into(),
            },
            FixtureX25519Keypair {
                pub_key: X25519_PUB3.into(),
                priv_key: X25519_SEC3.into(),
            },
        ],
    )
    .await?;
    Ok(api)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
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

            let agent_pubkey3 = holo_hash::AgentPubKey::new_from_pure_entropy(&keystore)
                .await
                .unwrap();

            #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
            struct MyData(Vec<u8>);

            let my_data = MyData(b"signature test data 1".to_vec());

            let signature1 = agent_pubkey1.sign(&keystore, &my_data).await.unwrap();
            assert!(agent_pubkey1
                .verify_signature(&signature1, &my_data)
                .await
                .unwrap());

            let signature2 = agent_pubkey2.sign(&keystore, &my_data).await.unwrap();
            assert!(agent_pubkey2
                .verify_signature(&signature2, &my_data)
                .await
                .unwrap());

            let signature3 = agent_pubkey3.sign(&keystore, &my_data).await.unwrap();
            assert!(agent_pubkey3
                .verify_signature(&signature3, &my_data)
                .await
                .unwrap());
        })
        .await
        .unwrap();
    }
}
