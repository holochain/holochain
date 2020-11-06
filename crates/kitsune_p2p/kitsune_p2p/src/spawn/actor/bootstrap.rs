use once_cell::sync::Lazy;

static CLIENT: Lazy<reqwest::Client> = Lazy::new(reqwest::Client::new);

/// @todo make this not hardcoded.
/// @todo handle testing vs. production better somehow.
const BOOTSTRAP_URL: &str = "https://bootstrap.holo.host";

const OP_HEADER: &str = "X-Op";
const OP_PUT: &str = "put";

pub async fn put(
    agent_info_signed: crate::types::agent_store::AgentInfoSigned,
) -> crate::types::actor::KitsuneP2pResult<()> {
    let mut data = Vec::new();
    kitsune_p2p_types::codec::rmp_encode(&mut data, &agent_info_signed)?;
    let res = CLIENT
        .post(BOOTSTRAP_URL)
        .body(data)
        .header(OP_HEADER, OP_PUT)
        .header(reqwest::header::CONTENT_TYPE, "application/octet")
        .send()
        .await?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(crate::KitsuneP2pError::Bootstrap(std::sync::Arc::new(
            res.text().await?,
        )))
    }
}

#[cfg(test)]
mod tests {

    use crate::fixt::*;
    use crate::types::agent_store::*;
    use crate::types::KitsuneAgent;
    use crate::types::KitsuneSignature;
    use fixt::prelude::*;
    use lair_keystore_api::internal::sign_ed25519::sign_ed25519_keypair_new_from_entropy;
    use std::convert::TryInto;

    #[tokio::test(threaded_scheduler)]
    async fn test_bootstrap() {
        let keypair = sign_ed25519_keypair_new_from_entropy().await.unwrap();
        let space = fixt!(KitsuneSpace);
        let agent = KitsuneAgent((*keypair.pub_key.0).clone());
        let urls = fixt!(Urls);
        let now = std::time::SystemTime::now();
        let millis = now
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();
        let agent_info = AgentInfo::new(
            space,
            agent.clone(),
            urls,
            (millis - 100).try_into().unwrap(),
        );
        let mut data = Vec::new();
        kitsune_p2p_types::codec::rmp_encode(&mut data, &agent_info).unwrap();
        let signature = keypair
            .sign(std::sync::Arc::new(data.clone()))
            .await
            .unwrap();
        let agent_info_signed =
            AgentInfoSigned::try_new(agent, KitsuneSignature((*signature.0).clone()), data)
                .unwrap();

        // Simply hitting the endpoint should be OK.
        super::put(agent_info_signed).await.unwrap();

        // We should get back an error if we don't have a good signature.
        assert!(super::put(fixt!(AgentInfoSigned)).await.is_err());
    }
}
