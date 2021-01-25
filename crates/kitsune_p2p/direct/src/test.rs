use crate::*;

#[tokio::test(threaded_scheduler)]
async fn sanity() {
    observability::test_run().ok();

    if let Err(e) = sanity_inner().await {
        panic!("{:#?}", e);
    }
}

async fn sanity_inner() -> KdResult<()> {
    let kd1 = spawn_kitsune_p2p_direct(KdConfig {
        persist_path: None,
        unlock_passphrase: sodoken::Buffer::new_memlocked(4)?,
        directives: vec![
            "set_proxy_accept_all:".to_string(),
            "bind_mem_local:".to_string(),
        ],
    })
    .await?;
    let kd2 = spawn_kitsune_p2p_direct(KdConfig {
        persist_path: None,
        unlock_passphrase: sodoken::Buffer::new_memlocked(4)?,
        directives: vec![
            "set_proxy_accept_all:".to_string(),
            "bind_mem_local:".to_string(),
        ],
    })
    .await?;

    let url1 = kd1.list_transport_bindings().await?[0].clone();
    println!("got connection: {:?}", url1);
    let url2 = kd2.list_transport_bindings().await?[0].clone();
    println!("got connection: {:?}", url2);

    let agent1 = kd1.generate_agent().await?;
    println!("got agent: {}", agent1);
    let agent2 = kd2.generate_agent().await?;
    println!("got agent: {}", agent2);

    kd1.join(agent1.clone(), agent1.clone()).await?;
    let info1 = kd1.list_known_agent_info(agent1.clone()).await?;
    println!("1:agent_info: {:?}", info1);

    kd2.create_entry(
        agent1.clone(),
        agent2.clone(),
        KdEntry::builder()
            .set_sys_type(SysType::Create)
            .set_expire(chrono::MAX_DATETIME)
            .set_left_link(&agent1),
    )
    .await?;

    kd2.join(agent1.clone(), agent2.clone()).await?;
    kd2.inject_agent_info(agent1.clone(), info1).await?;

    let mut recv = kd1.activate(agent1.clone()).await?;
    tokio::task::spawn(async move {
        use tokio::stream::StreamExt;
        while let Some(evt) = recv.next().await {
            println!("1:GOT: {:?}", evt);
        }
    });

    let mut recv = kd2.activate(agent2.clone()).await?;
    tokio::task::spawn(async move {
        use tokio::stream::StreamExt;
        while let Some(evt) = recv.next().await {
            println!("2:GOT: {:?}", evt);
        }
    });

    kd1.message(
        agent1.clone(),
        agent1.clone(),
        agent1.clone(),
        serde_json::json! {{
            "test": "message",
            "age": 42
        }},
    )
    .await?;

    kd2.message(
        agent1.clone(),
        agent2.clone(),
        agent2.clone(),
        serde_json::json! {{
            "test": "message2",
            "age": 43
        }},
    )
    .await?;

    tokio::time::delay_for(std::time::Duration::from_secs(3)).await;

    Ok(())
}
