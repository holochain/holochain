use super::*;
use crate::test_util::data::mk_agent_info;

struct TestInner<L, A>
where
    L: Fn(Arc<KitsuneAgent>) -> KitsuneP2pResult<bool> + 'static + Send + Sync,
    A: Fn(
            Arc<KitsuneAgent>,
        ) -> Result<Option<AgentInfoSigned>, Box<dyn Send + Sync + std::error::Error>>
        + 'static
        + Send
        + Sync,
{
    l: L,
    a: A,
}

impl<L, A> SearchAndDiscoverPeerConnect for TestInner<L, A>
where
    L: Fn(Arc<KitsuneAgent>) -> KitsuneP2pResult<bool> + 'static + Send + Sync,
    A: Fn(
            Arc<KitsuneAgent>,
        ) -> Result<Option<AgentInfoSigned>, Box<dyn Send + Sync + std::error::Error>>
        + 'static
        + Send
        + Sync,
{
    fn is_agent_local(
        &self,
        agent: Arc<KitsuneAgent>,
    ) -> MustBoxFuture<'static, KitsuneP2pResult<bool>> {
        let res = (self.l)(agent);
        async move { res }.boxed().into()
    }

    fn get_agent_info_signed(
        &self,
        agent: Arc<KitsuneAgent>,
    ) -> MustBoxFuture<'_, Result<Option<AgentInfoSigned>, Box<dyn Send + Sync + std::error::Error>>>
    {
        let res = (self.a)(agent);
        async move { res }.boxed().into()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn is_local() {
    let mut logic = SearchAndDiscoverPeerConnectLogic::new(
        TestInner {
            l: |_a| Ok(true),
            a: |_a| unreachable!(),
        },
        1,
        1,
        Arc::new(KitsuneAgent::new(vec![6; 32])),
        KitsuneTimeout::from_millis(1000),
    );

    assert!(matches!(
        logic.check_state().await,
        SearchAndDiscoverPeerConnectLogicResult::ShouldReturn(PeerDiscoverResult::OkShortcut)
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn have_info() {
    let agent_info = std::sync::Mutex::new(Some(mk_agent_info(6).await));

    let mut logic = SearchAndDiscoverPeerConnectLogic::new(
        TestInner {
            l: |_a| Ok(false),
            a: move |_a| Ok(agent_info.lock().unwrap().take()),
        },
        1,
        1,
        Arc::new(KitsuneAgent::new(vec![6; 32])),
        KitsuneTimeout::from_millis(1000),
    );

    assert!(matches!(
        logic.check_state().await,
        SearchAndDiscoverPeerConnectLogicResult::ShouldPeerConnect(_)
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn ignore_local_err() {
    let agent_info = std::sync::Mutex::new(Some(mk_agent_info(6).await));

    let mut logic = SearchAndDiscoverPeerConnectLogic::new(
        TestInner {
            l: |_a| Err("err".into()),
            a: move |_a| Ok(agent_info.lock().unwrap().take()),
        },
        1,
        1,
        Arc::new(KitsuneAgent::new(vec![6; 32])),
        KitsuneTimeout::from_millis(1000),
    );

    assert!(matches!(
        logic.check_state().await,
        SearchAndDiscoverPeerConnectLogicResult::ShouldPeerConnect(_)
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn should_search_peers() {
    let mut logic = SearchAndDiscoverPeerConnectLogic::new(
        TestInner {
            l: |_a| Ok(false),
            a: |_a| Ok(None),
        },
        1,
        1,
        Arc::new(KitsuneAgent::new(vec![6; 32])),
        KitsuneTimeout::from_millis(1000),
    );

    assert!(matches!(
        logic.check_state().await,
        SearchAndDiscoverPeerConnectLogicResult::ShouldSearchPeers
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn timeout() {
    let mut logic = SearchAndDiscoverPeerConnectLogic::new(
        TestInner {
            l: |_a| Ok(false),
            a: |_a| Ok(None),
        },
        1,
        1,
        Arc::new(KitsuneAgent::new(vec![6; 32])),
        KitsuneTimeout::from_millis(1),
    );

    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    assert!(matches!(
        logic.check_state().await,
        SearchAndDiscoverPeerConnectLogicResult::ShouldReturn(PeerDiscoverResult::Err(_))
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn ignore_peer_get_error() {
    let mut logic = SearchAndDiscoverPeerConnectLogic::new(
        TestInner {
            l: |_a| Ok(false),
            a: |_a| Err("err".into()),
        },
        1,
        1,
        Arc::new(KitsuneAgent::new(vec![6; 32])),
        KitsuneTimeout::from_millis(1000),
    );

    assert!(matches!(
        logic.check_state().await,
        SearchAndDiscoverPeerConnectLogicResult::ShouldSearchPeers
    ));
}
