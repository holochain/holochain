use super::*;
use kitsune_p2p_types::tx2::tx2_utils::TxUrl;
use SearchRemotesCoveringBasisLogicResult::*;

async fn mk_agent_info(u: u8, covers: u32, offline: bool) -> AgentInfoSigned {
    let url_list = if offline {
        vec![]
    } else {
        vec![TxUrl::from_str_panicking("wss://test")]
    };

    AgentInfoSigned::sign(
        Arc::new(KitsuneSpace::new(vec![0x11; 32])),
        Arc::new(KitsuneAgent::new(vec![u; 32])),
        covers,
        url_list,
        0,
        0,
        |_| async move { Ok(Arc::new(KitsuneSignature(vec![0; 64]))) },
    )
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn happy_path() {
    let mut logic = SearchRemotesCoveringBasisLogic::new(
        1,
        1,
        2,
        (u32::MAX / 4).into(),
        KitsuneTimeout::from_millis(1000),
    );

    assert!(matches!(logic.check_nodes(vec![]), ShouldWait));

    let near = mk_agent_info(1, 1, false).await;

    assert!(matches!(logic.check_nodes(vec![near]), QueryPeers(_)));

    let covers = mk_agent_info(2, u32::MAX, false).await;

    assert!(matches!(logic.check_nodes(vec![covers]), Success(_)));
}

#[tokio::test(flavor = "multi_thread")]
async fn timeout() {
    let mut logic = SearchRemotesCoveringBasisLogic::new(
        1,
        1,
        2,
        (u32::MAX / 4).into(),
        KitsuneTimeout::from_millis(1),
    );

    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    assert!(matches!(logic.check_nodes(vec![]), Error(_)));
}

#[tokio::test(flavor = "multi_thread")]
async fn respect_max_covers() {
    let mut logic = SearchRemotesCoveringBasisLogic::new(
        1,
        1,
        2,
        (u32::MAX / 4).into(),
        KitsuneTimeout::from_millis(1000),
    );

    let mut covers = Vec::new();
    for i in 0..5 {
        covers.push(mk_agent_info(i, u32::MAX, false).await);
    }

    assert!(matches!(
        logic.check_nodes(covers),
        Success(results) if results.len() == 2
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn respect_max_near() {
    let mut logic = SearchRemotesCoveringBasisLogic::new(
        1,
        1,
        2,
        (u32::MAX / 4).into(),
        KitsuneTimeout::from_millis(1000),
    );

    let mut covers = Vec::new();
    for i in 0..5 {
        covers.push(mk_agent_info(i, 1, false).await);
    }

    assert!(matches!(
        logic.check_nodes(covers),
        QueryPeers(results) if results.len() == 2
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn ignore_offline_nodes() {
    let mut logic = SearchRemotesCoveringBasisLogic::new(
        1,
        1,
        2,
        (u32::MAX / 4).into(),
        KitsuneTimeout::from_millis(1000),
    );

    let covers_offline = mk_agent_info(2, u32::MAX, true).await;

    assert!(matches!(
        logic.check_nodes(vec![covers_offline]),
        ShouldWait
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn ignore_zero_cover_nodes() {
    let mut logic = SearchRemotesCoveringBasisLogic::new(
        1,
        1,
        2,
        (u32::MAX / 4).into(),
        KitsuneTimeout::from_millis(1000),
    );

    let mut nodes = Vec::new();

    // this one just has a small arc
    nodes.push(mk_agent_info(2, 1, false).await);
    // this one is a full-on lurker
    nodes.push(mk_agent_info(2, 0, false).await);

    // don't bother querying lurkers
    assert!(matches!(
        logic.check_nodes(nodes),
        QueryPeers(results) if results.len() == 1
    ));
}
