#![allow(clippy::field_reassign_with_default)]
use futures::future::FutureExt;
use kitsune_p2p_direct::dependencies::*;
use kitsune_p2p_direct::prelude::*;
use kitsune_p2p_direct_test::direct_test_local_periodic::*;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    init_tracing();

    let mut config = KdTestConfig::default();
    config.node_count = 10;
    config.agents_per_node = 10;
    config.periodic_agent_hook_interval_ms = Some(1000);
    config.periodic_agent_hook = Box::new(|input| {
        async move {
            let AgentHookInput {
                root,
                app_entry_hash,
                agent,
                kdirect: _,
                kdhnd,
            } = input;

            let new_entry = KdEntryContent {
                kind: "u.foo".to_string(),
                parent: app_entry_hash,
                author: agent.clone(),
                verify: "".to_string(),
                data: serde_json::json!({
                    "nonce": std::time::SystemTime::now()
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs_f64(),
                }),
            };
            let new_entry = kdhnd
                .entry_author(
                    root.clone(),
                    agent.clone(),
                    new_entry,
                    vec![].into_boxed_slice().into(),
                )
                .await?;
            tracing::debug!(?new_entry);

            Ok(())
        }
        .boxed()
    });

    let test = KdTestHarness::start_test(config).await.unwrap();

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        let mut entry_counts = Vec::new();
        for node in test.nodes.iter() {
            for agent in node.local_agents.iter() {
                let entry_count = node
                    .kdirect
                    .get_persist()
                    .query_entries(
                        test.root.clone(),
                        agent.clone(),
                        f32::MIN,
                        f32::MAX,
                        DhtArc::new(0, u32::MAX),
                    )
                    .await
                    .unwrap()
                    .len();
                entry_counts.push(entry_count);
            }
        }
        println!("## ENTRY COUNTS: {:?}", entry_counts);
    }
}
