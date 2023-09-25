use super::*;
use crate::test_util::data::mk_agent_info;

#[tokio::test(flavor = "multi_thread")]
async fn happy_path() {
    struct T;

    impl GetCachedRemotesNearBasisSpace for T {
        fn space(&self) -> Arc<KitsuneSpace> {
            Arc::new(KitsuneSpace::new(vec![0x11; 32]))
        }

        fn query_agents(
            &self,
            _query: QueryAgentsEvt,
        ) -> MustBoxFuture<'static, KitsuneP2pResult<Vec<AgentInfoSigned>>> {
            async move {
                Ok(vec![
                    mk_agent_info(1).await,
                    mk_agent_info(2).await,
                    mk_agent_info(3).await,
                ])
            }
            .boxed()
            .into()
        }

        fn is_agent_local(
            &self,
            _agent: Arc<KitsuneAgent>,
        ) -> MustBoxFuture<'static, KitsuneP2pResult<bool>> {
            async move { Ok(false) }.boxed().into()
        }
    }

    assert_eq!(
        3,
        get_cached_remotes_near_basis(T, 0.into(), KitsuneTimeout::from_millis(100))
            .await
            .unwrap()
            .len()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn is_agent_local_err() {
    struct T;

    impl GetCachedRemotesNearBasisSpace for T {
        fn space(&self) -> Arc<KitsuneSpace> {
            Arc::new(KitsuneSpace::new(vec![0x11; 32]))
        }

        fn query_agents(
            &self,
            _query: QueryAgentsEvt,
        ) -> MustBoxFuture<'static, KitsuneP2pResult<Vec<AgentInfoSigned>>> {
            async move { Ok(vec![mk_agent_info(1).await]) }
                .boxed()
                .into()
        }

        fn is_agent_local(
            &self,
            _agent: Arc<KitsuneAgent>,
        ) -> MustBoxFuture<'static, KitsuneP2pResult<bool>> {
            async move { Err(KitsuneP2pError::other("yo")) }
                .boxed()
                .into()
        }
    }

    assert!(
        get_cached_remotes_near_basis(T, 0.into(), KitsuneTimeout::from_millis(100))
            .await
            .is_err()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn removes_locals() {
    struct T;

    impl GetCachedRemotesNearBasisSpace for T {
        fn space(&self) -> Arc<KitsuneSpace> {
            Arc::new(KitsuneSpace::new(vec![0x11; 32]))
        }

        fn query_agents(
            &self,
            _query: QueryAgentsEvt,
        ) -> MustBoxFuture<'static, KitsuneP2pResult<Vec<AgentInfoSigned>>> {
            async move {
                let mut out = Vec::new();
                for i in 0..40 {
                    out.push(mk_agent_info(i).await);
                }
                Ok(out)
            }
            .boxed()
            .into()
        }

        fn is_agent_local(
            &self,
            agent: Arc<KitsuneAgent>,
        ) -> MustBoxFuture<'static, KitsuneP2pResult<bool>> {
            async move { Ok(agent[0] >= 20) }.boxed().into()
        }
    }

    assert_eq!(
        20,
        get_cached_remotes_near_basis(T, 0.into(), KitsuneTimeout::from_millis(100))
            .await
            .unwrap()
            .len()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_is_err() {
    struct T;

    impl GetCachedRemotesNearBasisSpace for T {
        fn space(&self) -> Arc<KitsuneSpace> {
            Arc::new(KitsuneSpace::new(vec![0x11; 32]))
        }

        fn query_agents(
            &self,
            _query: QueryAgentsEvt,
        ) -> MustBoxFuture<'static, KitsuneP2pResult<Vec<AgentInfoSigned>>> {
            async move { Ok(vec![]) }.boxed().into()
        }

        fn is_agent_local(
            &self,
            _agent: Arc<KitsuneAgent>,
        ) -> MustBoxFuture<'static, KitsuneP2pResult<bool>> {
            async move { Ok(false) }.boxed().into()
        }
    }

    assert!(
        get_cached_remotes_near_basis(T, 0.into(), KitsuneTimeout::from_millis(100))
            .await
            .is_err()
    );
}
