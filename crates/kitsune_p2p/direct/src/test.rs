use crate::prelude::*;
use futures::stream::StreamExt;
use kitsune_p2p_direct_api::kd_sys_kind::{self, *};

#[tokio::test(flavor = "multi_thread")]
async fn test_direct_sanity() {
    let (bootstrap, driver, bootstrap_close) = new_quick_bootstrap_v1(Default::default()).await.unwrap();
    tokio::task::spawn(driver);

    let (proxy, driver, proxy_close) = new_quick_proxy_v1(Default::default()).await.unwrap();
    tokio::task::spawn(driver);

    let conf = KitsuneDirectV1Config {
        tuning_params: Default::default(),
        persist: new_persist_mem(),
        bootstrap,
        proxy,
        ui_port: 0,
    };

    let (kd, driver) = new_kitsune_direct_v1(conf).await.unwrap();

    tokio::task::spawn(driver);

    let ws_addr = kd.get_ui_addr().unwrap();
    println!("ws_addr: {}", ws_addr);

    let pass = sodoken::Buffer::new_memlocked(4).unwrap();
    pass.write_lock().copy_from_slice(&[1, 2, 3, 4]);

    let (hnd, mut hnd_evt) = new_handle_ws(ws_addr, pass).await.unwrap();

    let (m_s, mut m_r) = tokio::sync::mpsc::channel(32);
    tokio::task::spawn(async move {
        while let Some(evt) = hnd_evt.next().await {
            m_s.send(evt).await.unwrap();
        }
    });

    let root = hnd.keypair_get_or_create_tagged("test_root").await.unwrap();
    println!("got root app hash: {}", root);

    let agent1 = hnd
        .keypair_get_or_create_tagged("test_agent1")
        .await
        .unwrap();
    println!("got agent1 pubkey: {}", agent1);

    let agent2 = hnd
        .keypair_get_or_create_tagged("test_agent2")
        .await
        .unwrap();
    println!("got agent2 pubkey: {}", agent2);

    hnd.app_join(root.clone(), agent1.clone()).await.unwrap();
    let ai_1 = hnd
        .agent_info_get(root.clone(), agent1.clone())
        .await
        .unwrap();
    println!("agent info 1: {}", ai_1.to_string());

    hnd.app_join(root.clone(), agent2.clone()).await.unwrap();
    let ai_2 = hnd
        .agent_info_get(root.clone(), agent2.clone())
        .await
        .unwrap();
    println!("agent info 2: {}", ai_2.to_string());

    // this won't actually do anything... but make sure it doesn't error
    hnd.agent_info_store(ai_1.clone()).await.unwrap();

    let all_ai = hnd.agent_info_query(root.clone()).await.unwrap();
    assert_eq!(2, all_ai.len());
    for ai in all_ai {
        if ai.agent() != ai_1.agent() && ai.agent() != ai_2.agent() {
            panic!("unknown agent_info: {}", ai.to_string());
        }
    }

    hnd.message_send(
        root.clone(),
        agent2.clone(),
        agent1.clone(),
        serde_json::json!({ "test": "hello" }),
        vec![1, 1, 2, 2].into_boxed_slice().into(),
    )
    .await
    .unwrap();

    let evt = m_r.recv().await.unwrap();
    println!("GOT HND EVT: {:#?}", evt);

    let app_entry = hnd
        .entry_author(
            root.clone(),
            root.clone(),
            KdEntryContent {
                kind: "s.app".to_string(),
                parent: root.clone(),
                author: root.clone(),
                verify: "".to_string(),
                data: kd_sys_kind::KdSysKindApp {
                    name: "test".to_string(),
                }
                .to_json()
                .unwrap(),
            },
            vec![3, 3, 4, 4].into_boxed_slice().into(),
        )
        .await
        .unwrap();
    println!("signed_entry: {}", app_entry.to_string());

    let e = hnd
        .entry_get(root.clone(), root.clone(), app_entry.hash().clone())
        .await
        .unwrap();

    assert_eq!(app_entry, e);

    bootstrap_close(0, "").await;
    proxy_close(0, "").await;
    hnd.close(0, "").await;
    kd.close(0, "").await;
}
