use super::*;
use kitsune_p2p_types::{agent_info::AgentInfoSigned, dht_arc::DhtLocation};
use std::future::Future;

/// This enum represents the outcomes from peer discovery
/// - OkShortcut - the agent is locally joined, just mirror the request back out
/// - OkRemote - we were able to successfully establish a remote connection
/// - Err - we were not able to establish a connection within the timeout
pub(crate) enum PeerDiscoverResult {
    OkShortcut,
    OkRemote {
        #[allow(dead_code)]
        url: String,
        con_hnd: MetaNetCon,
    },
    Err(KitsuneP2pError),
}

pub(crate) trait SearchAndDiscoverPeerConnect: 'static + Send + Sync {
    fn is_agent_local(
        &self,
        agent: Arc<KitsuneAgent>,
    ) -> MustBoxFuture<'static, KitsuneP2pResult<bool>>;

    fn get_agent_info_signed(
        &self,
        agent: Arc<KitsuneAgent>,
    ) -> MustBoxFuture<'_, Result<Option<AgentInfoSigned>, Box<dyn Send + Sync + std::error::Error>>>;
}

impl SearchAndDiscoverPeerConnect for Arc<SpaceReadOnlyInner> {
    fn is_agent_local(
        &self,
        agent: Arc<KitsuneAgent>,
    ) -> MustBoxFuture<'static, KitsuneP2pResult<bool>> {
        self.i_s.is_agent_local(agent)
    }

    fn get_agent_info_signed(
        &self,
        agent: Arc<KitsuneAgent>,
    ) -> MustBoxFuture<'_, Result<Option<AgentInfoSigned>, Box<dyn Send + Sync + std::error::Error>>>
    {
        self.host_api.get_agent_info_signed(GetAgentInfoSignedEvt {
            space: self.space.clone(),
            agent,
        })
    }
}

#[allow(clippy::enum_variant_names)]
pub(crate) enum SearchAndDiscoverPeerConnectLogicResult {
    ShouldReturn(PeerDiscoverResult),
    ShouldPeerConnect(AgentInfoSigned),
    ShouldSearchPeers,
}

pub(crate) struct SearchAndDiscoverPeerConnectLogic<S: SearchAndDiscoverPeerConnect> {
    inner: S,
    timeout: KitsuneTimeout,
    backoff: KitsuneBackoff,
    to_agent: Arc<KitsuneAgent>,
}

impl<S: SearchAndDiscoverPeerConnect> SearchAndDiscoverPeerConnectLogic<S> {
    pub fn new(
        inner: S,
        initial_delay_ms: u64,
        max_delay_ms: u64,
        to_agent: Arc<KitsuneAgent>,
        timeout: KitsuneTimeout,
    ) -> Self {
        let backoff = timeout.backoff(initial_delay_ms, max_delay_ms);
        Self {
            inner,
            timeout,
            backoff,
            to_agent,
        }
    }

    pub async fn wait(&self) {
        self.backoff.wait().await;
    }

    pub async fn check_state(&mut self) -> SearchAndDiscoverPeerConnectLogicResult {
        // see if the tgt agent is actually local
        if let Ok(true) = self.inner.is_agent_local(self.to_agent.clone()).await {
            return SearchAndDiscoverPeerConnectLogicResult::ShouldReturn(
                PeerDiscoverResult::OkShortcut,
            );
        }

        // see if we already know how to reach the tgt agent
        if let Ok(Some(agent_info_signed)) = self
            .inner
            .get_agent_info_signed(self.to_agent.clone())
            .await
        {
            return SearchAndDiscoverPeerConnectLogicResult::ShouldPeerConnect(agent_info_signed);
        }

        // the next step involves making network requests
        // so check our timeout first
        if self.timeout.is_expired() {
            return SearchAndDiscoverPeerConnectLogicResult::ShouldReturn(PeerDiscoverResult::Err(
                "timeout discovering peer".into(),
            ));
        }

        SearchAndDiscoverPeerConnectLogicResult::ShouldSearchPeers
    }
}

/// search for / discover / and open a connection to a specific remote agent
pub(crate) fn search_and_discover_peer_connect(
    inner: Arc<SpaceReadOnlyInner>,
    to_agent: Arc<KitsuneAgent>,
    timeout: KitsuneTimeout,
) -> impl Future<Output = PeerDiscoverResult> + 'static + Send {
    const INITIAL_DELAY_MS: u64 = 100;
    const MAX_DELAY_MS: u64 = 1000;

    let mut logic = SearchAndDiscoverPeerConnectLogic::new(
        inner.clone(),
        INITIAL_DELAY_MS,
        MAX_DELAY_MS,
        to_agent.clone(),
        timeout,
    );

    async move {
        loop {
            match logic.check_state().await {
                SearchAndDiscoverPeerConnectLogicResult::ShouldReturn(r) => return r,
                SearchAndDiscoverPeerConnectLogicResult::ShouldPeerConnect(agent_info_signed) => {
                    return peer_connect(inner, &agent_info_signed, timeout).await;
                }
                SearchAndDiscoverPeerConnectLogicResult::ShouldSearchPeers => (),
            }

            // let's do some discovery
            if let Ok(nodes) =
                search_remotes_covering_basis(inner.clone(), to_agent.get_loc(), timeout).await
            {
                for node in nodes {
                    // try connecting to the returned nodes
                    if let PeerDiscoverResult::OkRemote { con_hnd, .. } =
                        peer_connect(inner.clone(), &node, timeout).await
                    {
                        // make a peer query for the basis
                        let payload = wire::Wire::peer_get(inner.space.clone(), to_agent.clone());
                        match con_hnd.request(&payload, timeout).await {
                            Ok(wire::Wire::PeerGetResp(wire::PeerGetResp {
                                agent_info_signed: Some(agent_info_signed),
                            })) => {
                                if let Err(err) = inner
                                    .host_api
                                    .legacy
                                    .put_agent_info_signed(PutAgentInfoSignedEvt {
                                        space: inner.space.clone(),
                                        peer_data: vec![agent_info_signed.clone()],
                                    })
                                    .await
                                {
                                    tracing::error!(
                                        ?err,
                                        "search_and_discover_peer_connect: error putting agent info"
                                    );
                                }

                                // hey, we got our target node info
                                // return the try-to-connect future
                                return peer_connect(inner, &agent_info_signed, timeout).await;
                            }
                            Ok(wire::Wire::PeerGetResp(wire::PeerGetResp {
                                agent_info_signed: None,
                            })) => {
                                // No agent found, move on to the next node.
                                continue;
                            }
                            peer_resp => {
                                // This node is sending us something unexpected, so let's warn about that.
                                tracing::warn!(
                                    ?peer_resp,
                                    "search_and_discover_peer_connect: unexpected peer response"
                                );
                            }
                        }
                    }
                }
            }

            tracing::info!(
                "search_and_discover_peer_connect: no peers found, retrying after delay."
            );

            logic.wait().await;
        }
    }
}

/// attempt to establish a connection to another peer within given timeout
pub(crate) fn peer_connect(
    inner: Arc<SpaceReadOnlyInner>,
    agent_info_signed: &AgentInfoSigned,
    timeout: KitsuneTimeout,
) -> impl Future<Output = PeerDiscoverResult> + 'static + Send {
    let agent = agent_info_signed.agent.clone();
    let url = agent_info_signed
        .url_list
        .first()
        .cloned()
        .ok_or_else(|| KitsuneP2pError::from("no url - agent is likely offline"));

    async move {
        let url = url?;

        // if they are local, return the shortcut result
        if inner.i_s.is_agent_local(agent).await? {
            return Ok(PeerDiscoverResult::OkShortcut);
        }

        // attempt an outgoing connection
        let con_hnd = inner
            .ep_hnd
            .get_connection(url.to_string(), timeout)
            .await?;

        // return the result
        Ok(PeerDiscoverResult::OkRemote {
            url: url.to_string(),
            con_hnd,
        })
    }
    .map(|r| match r {
        Ok(r) => r,
        Err(e) => PeerDiscoverResult::Err(e),
    })
}

pub(crate) enum SearchRemotesCoveringBasisLogicResult {
    Success(Vec<AgentInfoSigned>),
    Error(KitsuneP2pError),
    ShouldWait,
    QueryPeers(Vec<AgentInfoSigned>),
}

pub(crate) struct SearchRemotesCoveringBasisLogic {
    timeout: KitsuneTimeout,
    backoff: KitsuneBackoff,
    check_node_count: usize,
    basis_loc: DhtLocation,
}

impl SearchRemotesCoveringBasisLogic {
    pub fn new(
        initial_delay_ms: u64,
        max_delay_ms: u64,
        check_node_count: usize,
        basis_loc: DhtLocation,
        timeout: KitsuneTimeout,
    ) -> Self {
        let backoff = timeout.backoff(initial_delay_ms, max_delay_ms);
        Self {
            timeout,
            backoff,
            check_node_count,
            basis_loc,
        }
    }

    pub async fn wait(&self) {
        self.backoff.wait().await;
    }

    pub fn check_nodes(
        &mut self,
        nodes: Vec<AgentInfoSigned>,
    ) -> SearchRemotesCoveringBasisLogicResult {
        let mut cover_nodes = Vec::new();
        let mut near_nodes = Vec::new();

        // first check our local peer store,
        // sort into nodes covering the basis and otherwise
        for node in nodes {
            // skip offline nodes
            if node.url_list.is_empty() {
                continue;
            }

            // skip nodes that can't tell us about any peers
            if node.storage_arc.range().is_empty() {
                continue;
            }

            if node.storage_arc.contains(self.basis_loc) {
                cover_nodes.push(node);
            } else {
                near_nodes.push(node);
            }

            if cover_nodes.len() + near_nodes.len() >= self.check_node_count {
                break;
            }
        }

        // if we have any nodes covering the basis, return them
        if !cover_nodes.is_empty() {
            return SearchRemotesCoveringBasisLogicResult::Success(cover_nodes);
        }

        // if we've exhausted our timeout, we should exit
        if let Err(err) = self.timeout.ok("search_remotes_covering_basis") {
            return SearchRemotesCoveringBasisLogicResult::Error(err.into());
        }

        if near_nodes.is_empty() {
            // maybe just wait and try again?
            return SearchRemotesCoveringBasisLogicResult::ShouldWait;
        }

        // shuffle the returned nodes so we don't keep hammering the same one
        use rand::prelude::*;
        near_nodes.shuffle(&mut rand::thread_rng());

        SearchRemotesCoveringBasisLogicResult::QueryPeers(near_nodes)
    }
}

/// looping search for agents covering basis_loc
/// by requesting closer agents from remote nodes
pub(crate) fn search_remotes_covering_basis(
    inner: Arc<SpaceReadOnlyInner>,
    basis_loc: DhtLocation,
    timeout: KitsuneTimeout,
) -> impl Future<Output = KitsuneP2pResult<Vec<AgentInfoSigned>>> + 'static + Send {
    const INITIAL_DELAY_MS: u64 = 100;
    const MAX_DELAY_MS: u64 = 1000;
    const CHECK_NODE_COUNT: usize = 8;

    let mut logic = SearchRemotesCoveringBasisLogic::new(
        INITIAL_DELAY_MS,
        MAX_DELAY_MS,
        CHECK_NODE_COUNT,
        basis_loc,
        timeout,
    );

    async move {
        loop {
            let nodes = get_cached_remotes_near_basis(inner.clone(), basis_loc, timeout)
                .await
                .unwrap_or_else(|_| Vec::new());

            let near_nodes = match logic.check_nodes(nodes) {
                SearchRemotesCoveringBasisLogicResult::Success(out) => {
                    return Ok(out);
                }
                SearchRemotesCoveringBasisLogicResult::Error(err) => {
                    return Err(err);
                }
                SearchRemotesCoveringBasisLogicResult::ShouldWait => {
                    logic.wait().await;
                    continue;
                }
                SearchRemotesCoveringBasisLogicResult::QueryPeers(nodes) => nodes,
            };

            let mut added_data = false;
            for node in near_nodes {
                // try connecting to the returned nodes
                if let PeerDiscoverResult::OkRemote { con_hnd, .. } =
                    peer_connect(inner.clone(), &node, timeout).await
                {
                    // make a peer query for the basis
                    let payload = wire::Wire::peer_query(inner.space.clone(), basis_loc);
                    match con_hnd.request(&payload, timeout).await {
                        Ok(wire::Wire::PeerQueryResp(wire::PeerQueryResp { peer_list })) => {
                            if peer_list.is_empty() {
                                tracing::warn!("empty discovery peer list");
                                continue;
                            }
                            // if we got results, add them to our peer store
                            if let Err(err) = inner
                                .host_api
                                .legacy
                                .put_agent_info_signed(PutAgentInfoSignedEvt {
                                    space: inner.space.clone(),
                                    peer_data: peer_list,
                                })
                                .await
                            {
                                tracing::error!(?err, "error storing peer_queried agent_info");
                            }
                            // then break, to pull up the local query
                            // that should now include these new results
                            added_data = true;
                            break;
                        }
                        peer_resp => {
                            tracing::warn!(
                                ?peer_resp,
                                "search_remotes_covering_basis: unexpected peer response"
                            );
                        }
                    }
                }
            }

            if !added_data {
                logic.wait().await;
            }
        }
    }
}

pub(crate) trait GetCachedRemotesNearBasisSpace: 'static + Send + Sync {
    fn space(&self) -> Arc<KitsuneSpace>;

    fn query_agents(
        &self,
        query: QueryAgentsEvt,
    ) -> MustBoxFuture<'static, KitsuneP2pResult<Vec<AgentInfoSigned>>>;

    fn is_agent_local(
        &self,
        agent: Arc<KitsuneAgent>,
    ) -> MustBoxFuture<'static, KitsuneP2pResult<bool>>;
}

impl GetCachedRemotesNearBasisSpace for Arc<SpaceReadOnlyInner> {
    fn space(&self) -> Arc<KitsuneSpace> {
        self.space.clone()
    }

    fn query_agents(
        &self,
        query: QueryAgentsEvt,
    ) -> MustBoxFuture<'static, KitsuneP2pResult<Vec<AgentInfoSigned>>> {
        self.host_api.legacy.query_agents(query)
    }

    fn is_agent_local(
        &self,
        agent: Arc<KitsuneAgent>,
    ) -> MustBoxFuture<'static, KitsuneP2pResult<bool>> {
        self.i_s.is_agent_local(agent)
    }
}

/// local search for remote (non-local) agents closest to basis
pub(crate) fn get_cached_remotes_near_basis<S: GetCachedRemotesNearBasisSpace>(
    inner: S,
    basis_loc: DhtLocation,
    _timeout: KitsuneTimeout,
) -> impl Future<Output = KitsuneP2pResult<Vec<AgentInfoSigned>>> + 'static + Send {
    // as this is a local request, there isn't much cost to getting more
    // results than we strictly need
    const LIMIT: u32 = 20;

    async move {
        let mut nodes = Vec::new();

        let query = QueryAgentsEvt::new(inner.space())
            .near_basis(basis_loc)
            .limit(LIMIT);
        for node in inner.query_agents(query).await? {
            if !inner.is_agent_local(node.agent.clone()).await? {
                nodes.push(node);
            }
        }

        if nodes.is_empty() {
            return Err("no remote nodes found, abort discovery".into());
        }

        Ok(nodes)
    }
}

#[cfg(test)]
mod test_search_and_discover_peer_connect;

#[cfg(test)]
mod test_search_remotes_covering_basis;

#[cfg(test)]
mod test_get_cached_remotes_near_basis;
