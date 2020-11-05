lazy_static::lazy_static! {
    static ref CLIENT: reqwest::Client = reqwest::Client::new();
}

/// @todo make this not hardcoded.
const BOOTSTRAP_URL: &str = "https://bootstrap.holo.host";

const OP_HEADER: &str = "X-Op";
const OP_PUT: &str = "put";

pub async fn put(
    agent_info_signed: crate::types::agent_store::AgentInfoSigned,
) -> crate::types::actor::KitsuneP2pResult<()> {
    let mut data = Vec::new();
    kitsune_p2p_types::codec::rmp_encode(&mut data, &agent_info_signed)?;
    CLIENT
        .post(BOOTSTRAP_URL)
        .body(data)
        .header(OP_HEADER, OP_PUT)
        .header(reqwest::header::CONTENT_TYPE, "application/octet")
        .send()
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {

    use crate::test_util::spawn_test_harness_mem;
    use crate::test_util::HarnessControlApiSender;

    #[tokio::test(threaded_scheduler)]
    async fn test_bootstrap() {
        // Simply joining a space with an agent will hit the bootstrap service internally.
        // Cross reference this with the values in the bootstrap service.
        let (harness, _) = spawn_test_harness_mem().await.unwrap();
        harness.add_space().await.unwrap();
        harness.add_direct_agent("alice".into()).await.unwrap();
    }
}
