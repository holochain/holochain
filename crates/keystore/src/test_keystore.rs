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

const CERT_SNI: &str = "ar1J-HVz0EO4CzS9CN8EFta.ad471maBa70w5vn6nNilfUa";
const CERT_SEC: &[u8] = &[
    48, 83, 2, 1, 1, 48, 5, 6, 3, 43, 101, 112, 4, 34, 4, 32, 135, 101, 23, 181, 167, 183, 114, 94,
    169, 84, 144, 224, 192, 41, 112, 118, 149, 226, 42, 187, 247, 210, 54, 43, 83, 125, 13, 209,
    93, 207, 33, 153, 161, 35, 3, 33, 0, 83, 74, 255, 70, 132, 118, 51, 92, 85, 250, 176, 123, 49,
    206, 237, 79, 161, 136, 99, 44, 52, 128, 94, 174, 55, 174, 198, 113, 79, 135, 111, 26,
];
const CERT: &[u8] = &[
    48, 130, 1, 48, 48, 129, 227, 160, 3, 2, 1, 2, 2, 1, 42, 48, 5, 6, 3, 43, 101, 112, 48, 33, 49,
    31, 48, 29, 6, 3, 85, 4, 3, 12, 22, 114, 99, 103, 101, 110, 32, 115, 101, 108, 102, 32, 115,
    105, 103, 110, 101, 100, 32, 99, 101, 114, 116, 48, 32, 23, 13, 55, 53, 48, 49, 48, 49, 48, 48,
    48, 48, 48, 48, 90, 24, 15, 52, 48, 57, 54, 48, 49, 48, 49, 48, 48, 48, 48, 48, 48, 90, 48, 33,
    49, 31, 48, 29, 6, 3, 85, 4, 3, 12, 22, 114, 99, 103, 101, 110, 32, 115, 101, 108, 102, 32,
    115, 105, 103, 110, 101, 100, 32, 99, 101, 114, 116, 48, 42, 48, 5, 6, 3, 43, 101, 112, 3, 33,
    0, 83, 74, 255, 70, 132, 118, 51, 92, 85, 250, 176, 123, 49, 206, 237, 79, 161, 136, 99, 44,
    52, 128, 94, 174, 55, 174, 198, 113, 79, 135, 111, 26, 163, 62, 48, 60, 48, 58, 6, 3, 85, 29,
    17, 4, 51, 48, 49, 130, 47, 97, 114, 49, 74, 45, 72, 86, 122, 48, 69, 79, 52, 67, 122, 83, 57,
    67, 78, 56, 69, 70, 116, 97, 46, 97, 100, 52, 55, 49, 109, 97, 66, 97, 55, 48, 119, 53, 118,
    110, 54, 110, 78, 105, 108, 102, 85, 97, 48, 5, 6, 3, 43, 101, 112, 3, 65, 0, 211, 114, 220,
    25, 145, 60, 41, 144, 219, 0, 170, 31, 206, 39, 134, 136, 147, 103, 63, 215, 239, 108, 28, 136,
    102, 40, 213, 247, 233, 32, 190, 66, 155, 175, 6, 206, 193, 223, 93, 244, 11, 54, 81, 66, 31,
    79, 20, 161, 138, 83, 58, 13, 4, 214, 204, 189, 12, 66, 180, 147, 202, 208, 242, 3,
];
const CERT_DIGEST: &[u8] = &[
    112, 155, 175, 48, 124, 184, 87, 220, 71, 56, 229, 88, 125, 146, 177, 13, 218, 216, 23, 59,
    225, 6, 23, 207, 126, 223, 169, 142, 92, 242, 240, 239,
];

const X25519_SEC: [u8; 32] = [
    253, 12, 117, 61, 12, 47, 207, 107, 110, 116, 6, 194, 214, 88, 61, 161,
    220, 6, 53, 190, 225, 254, 230, 143, 130, 70, 25, 160, 15, 168, 42, 37,
];
const X25519_PUB: [u8; 32] = [
    65, 17, 71, 31, 48, 10, 48, 208, 3, 220, 71, 246, 83, 246, 74, 221, 3,
    123, 54, 48, 160, 192, 179, 207, 115, 6, 19, 53, 233, 231, 167, 75,
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
        vec![FixtureTlsCert {
            priv_key_der: CERT_SEC.to_vec(),
            sni: CERT_SNI.to_string(),
            cert_der: CERT.to_vec(),
            cert_digest: CERT_DIGEST.to_vec(),
        }],
        vec![FixtureX25519Keypair {
            pub_key: X25519_PUB.into(),
            priv_key: X25519_SEC.into(),
        }],
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
