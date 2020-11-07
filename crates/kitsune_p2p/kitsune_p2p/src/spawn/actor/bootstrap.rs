use once_cell::sync::Lazy;

static CLIENT: Lazy<reqwest::Client> = Lazy::new(reqwest::Client::new);

const BOOTSTRAP_URL_ENV: &str = "P2P_BOOTSTRAP_URL";
const BOOTSTRAP_URL_DEFAULT: &str = "https://bootstrap.holo.host";
#[allow(dead_code)]
const BOOTSTRAP_URL_DEV: &str = "https://bootstrap-dev.holohost.workers.dev";

#[allow(clippy::declare_interior_mutable_const)]
const BOOTSTRAP_URL: Lazy<Option<String>> = Lazy::new(|| match std::env::var(BOOTSTRAP_URL_ENV) {
    Ok(v) => {
        // If the environment variable is set and empty then don't bootstrap at all.
        if v.is_empty() {
            None
        } else {
            Some(v)
        }
    }
    // If the environment variable is not set then fallback to the default.
    Err(_) => Some(BOOTSTRAP_URL_DEFAULT.to_string()),
});

const OP_HEADER: &str = "X-Op";
const OP_PUT: &str = "put";

pub async fn put(
    url_override: Option<String>,
    agent_info_signed: crate::types::agent_store::AgentInfoSigned,
) -> crate::types::actor::KitsuneP2pResult<()> {
    let url: Option<String> = match url_override {
        Some(url) => Some(url),
        #[allow(clippy::borrow_interior_mutable_const)]
        None => match Lazy::force(&BOOTSTRAP_URL) {
            Some(url) => Some(url.to_string()),
            None => None,
        },
    };

    match url {
        Some(url) => {
            let mut data = Vec::new();
            kitsune_p2p_types::codec::rmp_encode(&mut data, &agent_info_signed)?;
            let res = CLIENT
                .post(&url)
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
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {

    use crate::fixt::*;
    use crate::spawn::actor::space::AGENT_INFO_EXPIRES_AFTER_MS;
    use crate::types::agent_store::*;
    use crate::types::KitsuneAgent;
    use crate::types::KitsuneBinType;
    use crate::types::KitsuneSignature;
    use fixt::prelude::*;
    use lair_keystore_api::internal::sign_ed25519::sign_ed25519_keypair_new_from_entropy;
    use std::convert::TryInto;

    #[tokio::test(threaded_scheduler)]
    async fn test_bootstrap() {
        let keypair = sign_ed25519_keypair_new_from_entropy().await.unwrap();
        let space = fixt!(KitsuneSpace);
        let agent = KitsuneAgent::new((*keypair.pub_key.0).clone());
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
            AGENT_INFO_EXPIRES_AFTER_MS,
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
        super::put(
            Some(super::BOOTSTRAP_URL_DEV.to_string()),
            agent_info_signed,
        )
        .await
        .unwrap();

        // We should get back an error if we don't have a good signature.
        assert!(super::put(
            Some(super::BOOTSTRAP_URL_DEV.to_string()),
            fixt!(AgentInfoSigned)
        )
        .await
        .is_err());
    }
}
