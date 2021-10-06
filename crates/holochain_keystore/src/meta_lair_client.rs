use crate::*;
use kitsune_p2p_types::dependencies::new_lair_api;
use legacy_lair_api::actor::LairClientApiSender;
use new_lair_api::prelude::*;
use std::future::Future;

/// Abstraction around runtime switching/upgrade of lair keystore / client.
/// Can delete this when we finally delete deprecated legacy lair option.
#[derive(Clone)]
pub enum MetaLairClient {
    /// oldschool deprecated lair keystore client
    Legacy(KeystoreSender),

    /// new lair keystore api client
    NewLair(LairClient),
}

impl MetaLairClient {
    /// TODO this is just a temp helper to ease implementing the switch
    pub fn unwrap_legacy(&self) -> &KeystoreSender {
        if let MetaLairClient::Legacy(client) = self {
            client
        } else {
            todo!()
        }
    }

    /// Construct a new randomized signature keypair
    pub fn new_sign_keypair_random(
        &self,
    ) -> impl Future<Output = LairResult<holo_hash::AgentPubKey>> + 'static + Send {
        let this = self.clone();
        async move {
            match this {
                Self::Legacy(client) => {
                    let (_, pk) = client
                        .sign_ed25519_new_from_entropy()
                        .await
                        .map_err(one_err::OneErr::new)?;
                    Ok(holo_hash::AgentPubKey::from_raw_32(pk.to_vec()))
                }
                Self::NewLair(client) => {
                    let tag = nanoid::nanoid!();
                    let info = client.new_seed(tag.into(), None).await?;
                    let pub_key =
                        holo_hash::AgentPubKey::from_raw_32(info.ed25519_pub_key.0.to_vec());
                    Ok(pub_key)
                }
            }
        }
    }
}
