use super::*;

type KAgent = Arc<KitsuneAgent>;
type KInfo = Arc<AgentInfoSigned>;

ghost_actor::ghost_chan! {
    /// The api for the test harness controller
    pub chan HarnessControlApi<KitsuneP2pError> {
        /// Create a new random space id
        /// + join all existing harness agents to it
        /// + all new harness agents will also join it
        fn add_space() -> Arc<KitsuneSpace>;

        /// Create a new agent configured to proxy for others.
        fn add_proxy_agent(nick: String) -> (
            Arc<KitsuneAgent>,
            ghost_actor::GhostSender<KitsuneP2p>,
        );

        /// Create a new directly addressable agent that will
        /// reject any proxy requests.
        fn add_direct_agent(nick: String) -> (
            Arc<KitsuneAgent>,
            ghost_actor::GhostSender<KitsuneP2p>,
        );

        /// Create a new directly addressable agent that will
        /// reject any proxy requests.
        fn add_publish_only_agent(nick: String) -> (
            Arc<KitsuneAgent>,
            ghost_actor::GhostSender<KitsuneP2p>,
        );

        /// Create a new agent that will connect via proxy.
        fn add_nat_agent(nick: String, proxy_url: url2::Url2) -> (
            Arc<KitsuneAgent>,
            ghost_actor::GhostSender<KitsuneP2p>,
        );

        /// Magically exchange peer data between peers in harness
        fn magic_peer_info_exchange() -> ();

        /// Inject data for one specific agent to gossip to others
        fn inject_gossip_data(agent: KAgent, data: String) -> Arc<KitsuneOpHash>;

        /// Inject agent info for one agent.
        fn inject_peer_info(agent: KAgent, info: KInfo) -> ();

        /// Dump all local gossip data from a specific agent
        fn dump_local_gossip_data(agent: KAgent) -> HashMap<Arc<KitsuneOpHash>, String>;

        /// Dump all local peer data from a specific agent
        fn dump_local_peer_data(agent: KAgent) -> HashMap<Arc<KitsuneAgent>, Arc<AgentInfoSigned>>;
    }
}

/// construct a test suite around a mem transport
pub async fn spawn_test_harness_mem() -> Result<
    (
        ghost_actor::GhostSender<HarnessControlApi>,
        HarnessEventChannel,
    ),
    KitsuneP2pError,
> {
    spawn_test_harness(TransportConfig::Mem {}).await
}

/// construct a test suite around a quic transport
pub async fn spawn_test_harness_quic() -> Result<
    (
        ghost_actor::GhostSender<HarnessControlApi>,
        HarnessEventChannel,
    ),
    KitsuneP2pError,
> {
    spawn_test_harness(TransportConfig::Quic {
        bind_to: Some(url2::url2!("kitsune-quic://0.0.0.0:0")),
        override_host: None,
        override_port: None,
    })
    .await
}

/// construct a test suite around a sub transport config concept
pub async fn spawn_test_harness(
    sub_config: TransportConfig,
) -> Result<
    (
        ghost_actor::GhostSender<HarnessControlApi>,
        HarnessEventChannel,
    ),
    KitsuneP2pError,
> {
    let harness_chan = HarnessEventChannel::new("");

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let controller = builder
        .channel_factory()
        .create_channel::<HarnessControlApi>()
        .await?;

    let i_s = builder
        .channel_factory()
        .create_channel::<HarnessInner>()
        .await?;

    tokio::task::spawn(builder.spawn(HarnessActor::new(i_s, harness_chan.clone(), sub_config)));

    Ok((controller, harness_chan))
}

type KP2p = ghost_actor::GhostSender<KitsuneP2p>;
type KCtl = ghost_actor::GhostSender<HarnessAgentControl>;

ghost_actor::ghost_chan! {
    /// The api for the test harness controller
    chan HarnessInner<KitsuneP2pError> {
        fn finish_agent(
            agent: KAgent,
            p2p: KP2p,
            ctrl: KCtl,
        ) -> ();
    }
}

struct HarnessActor {
    i_s: ghost_actor::GhostSender<HarnessInner>,
    harness_chan: HarnessEventChannel,
    sub_config: TransportConfig,
    space_list: Vec<Arc<KitsuneSpace>>,
    agents: HashMap<
        Arc<KitsuneAgent>,
        (
            ghost_actor::GhostSender<KitsuneP2p>,
            ghost_actor::GhostSender<HarnessAgentControl>,
        ),
    >,
}

impl HarnessActor {
    pub fn new(
        i_s: ghost_actor::GhostSender<HarnessInner>,
        harness_chan: HarnessEventChannel,
        sub_config: TransportConfig,
    ) -> Self {
        Self {
            i_s,
            harness_chan,
            sub_config,
            space_list: Vec::new(),
            agents: HashMap::new(),
        }
    }
}

impl ghost_actor::GhostControlHandler for HarnessActor {
    fn handle_ghost_actor_shutdown(
        self,
    ) -> ghost_actor::dependencies::must_future::MustBoxFuture<'static, ()> {
        use ghost_actor::GhostControlSender;
        async move {
            self.harness_chan.close();
            for (_, (p2p, ctrl)) in self.agents.iter() {
                let _ = p2p.ghost_actor_shutdown().await;
                let _ = ctrl.ghost_actor_shutdown().await;
            }
        }
        .boxed()
        .into()
    }
}

impl ghost_actor::GhostHandler<HarnessInner> for HarnessActor {}

impl HarnessInnerHandler for HarnessActor {
    fn handle_finish_agent(
        &mut self,
        agent: Arc<KitsuneAgent>,
        p2p: ghost_actor::GhostSender<KitsuneP2p>,
        ctrl: ghost_actor::GhostSender<HarnessAgentControl>,
    ) -> HarnessInnerHandlerResult<()> {
        self.agents.insert(agent.clone(), (p2p.clone(), ctrl));

        let harness_chan = self.harness_chan.clone();
        let space_list = self.space_list.clone();
        Ok(async move {
            for space in space_list {
                p2p.join(space.clone(), agent.clone()).await?;

                harness_chan.publish(HarnessEventType::Join {
                    agent: (&agent).into(),
                    space: space.into(),
                });
            }
            Ok(())
        }
        .boxed()
        .into())
    }
}

impl ghost_actor::GhostHandler<HarnessControlApi> for HarnessActor {}

impl HarnessControlApiHandler for HarnessActor {
    fn handle_add_space(&mut self) -> HarnessControlApiHandlerResult<Arc<KitsuneSpace>> {
        let space: Arc<KitsuneSpace> = TestVal::test_val();
        self.space_list.push(space.clone());
        let mut all = Vec::new();
        for (agent, (p2p, _)) in self.agents.iter() {
            all.push(p2p.join(space.clone(), agent.clone()));
        }
        Ok(async move {
            futures::future::try_join_all(all).await?;
            Ok(space)
        }
        .boxed()
        .into())
    }

    fn handle_add_proxy_agent(
        &mut self,
        nick: String,
    ) -> HarnessControlApiHandlerResult<(Arc<KitsuneAgent>, ghost_actor::GhostSender<KitsuneP2p>)>
    {
        let mut proxy_agent_config = KitsuneP2pConfig::default();
        proxy_agent_config
            .transport_pool
            .push(TransportConfig::Proxy {
                sub_transport: Box::new(self.sub_config.clone()),
                proxy_config: ProxyConfig::LocalProxyServer {
                    proxy_accept_config: Some(ProxyAcceptConfig::AcceptAll),
                },
            });

        let sub_harness = self.harness_chan.sub_clone(nick);
        let i_s = self.i_s.clone();
        Ok(async move {
            let (agent, p2p, ctrl) = spawn_test_agent(sub_harness, proxy_agent_config).await?;

            i_s.finish_agent(agent.clone(), p2p.clone(), ctrl).await?;

            Ok((agent, p2p))
        }
        .boxed()
        .into())
    }

    fn handle_add_direct_agent(
        &mut self,
        nick: String,
    ) -> HarnessControlApiHandlerResult<(Arc<KitsuneAgent>, ghost_actor::GhostSender<KitsuneP2p>)>
    {
        let mut direct_agent_config = KitsuneP2pConfig::default();
        direct_agent_config
            .transport_pool
            .push(TransportConfig::Proxy {
                sub_transport: Box::new(self.sub_config.clone()),
                proxy_config: ProxyConfig::LocalProxyServer {
                    proxy_accept_config: Some(ProxyAcceptConfig::RejectAll),
                },
            });

        let sub_harness = self.harness_chan.sub_clone(nick);
        let i_s = self.i_s.clone();
        Ok(async move {
            let (agent, p2p, ctrl) = spawn_test_agent(sub_harness, direct_agent_config).await?;

            i_s.finish_agent(agent.clone(), p2p.clone(), ctrl).await?;

            Ok((agent, p2p))
        }
        .boxed()
        .into())
    }

    fn handle_add_publish_only_agent(
        &mut self,
        nick: String,
    ) -> HarnessControlApiHandlerResult<(Arc<KitsuneAgent>, ghost_actor::GhostSender<KitsuneP2p>)>
    {
        let mut tp =
            kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
        tp.gossip_strategy = "none".to_string();
        let tp = Arc::new(tp);
        let mut direct_agent_config = KitsuneP2pConfig {
            tuning_params: tp,
            ..Default::default()
        };
        direct_agent_config
            .transport_pool
            .push(TransportConfig::Proxy {
                sub_transport: Box::new(self.sub_config.clone()),
                proxy_config: ProxyConfig::LocalProxyServer {
                    proxy_accept_config: Some(ProxyAcceptConfig::RejectAll),
                },
            });

        let sub_harness = self.harness_chan.sub_clone(nick);
        let i_s = self.i_s.clone();
        Ok(async move {
            let (agent, p2p, ctrl) = spawn_test_agent(sub_harness, direct_agent_config).await?;

            i_s.finish_agent(agent.clone(), p2p.clone(), ctrl).await?;

            Ok((agent, p2p))
        }
        .boxed()
        .into())
    }

    fn handle_add_nat_agent(
        &mut self,
        nick: String,
        proxy_url: url2::Url2,
    ) -> HarnessControlApiHandlerResult<(Arc<KitsuneAgent>, ghost_actor::GhostSender<KitsuneP2p>)>
    {
        let mut nat_agent_config = KitsuneP2pConfig::default();
        nat_agent_config
            .transport_pool
            .push(TransportConfig::Proxy {
                sub_transport: Box::new(self.sub_config.clone()),
                proxy_config: ProxyConfig::RemoteProxyClient { proxy_url },
            });

        let sub_harness = self.harness_chan.sub_clone(nick);
        let i_s = self.i_s.clone();
        Ok(async move {
            let (agent, p2p, ctrl) = spawn_test_agent(sub_harness, nat_agent_config).await?;

            i_s.finish_agent(agent.clone(), p2p.clone(), ctrl).await?;

            Ok((agent, p2p))
        }
        .boxed()
        .into())
    }

    fn handle_magic_peer_info_exchange(&mut self) -> HarnessControlApiHandlerResult<()> {
        let ctrls = self
            .agents
            .values()
            .map(|(_, ctrl)| ctrl.clone())
            .collect::<Vec<_>>();

        Ok(async move {
            let infos = ctrls.iter().map(|c| c.dump_agent_info());
            let infos = futures::future::try_join_all(infos).await?;
            let infos = infos.into_iter().fold(HashMap::new(), |acc, x| {
                x.into_iter().fold(acc, |mut acc, x| {
                    acc.insert(x.agent.clone(), x);
                    acc
                })
            });
            let infos = ctrls.iter().map(|c| c.inject_agent_info(infos.clone()));
            futures::future::try_join_all(infos).await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_inject_gossip_data(
        &mut self,
        agent: Arc<KitsuneAgent>,
        data: String,
    ) -> HarnessControlApiHandlerResult<Arc<KitsuneOpHash>> {
        let (_, ctrl) = self
            .agents
            .get(&agent)
            .ok_or_else(|| KitsuneP2pError::from("invalid agent"))?;
        let fut = ctrl.inject_gossip_data(data);
        Ok(async move { fut.await }.boxed().into())
    }

    fn handle_inject_peer_info(
        &mut self,
        agent: KAgent,
        info: KInfo,
    ) -> HarnessControlApiHandlerResult<()> {
        let (_, ctrl) = self
            .agents
            .get(&agent)
            .ok_or_else(|| KitsuneP2pError::from("invalid agent"))?;
        let map = maplit::hashmap! {
            info.agent.clone() => info
        };
        let fut = ctrl.inject_agent_info(map);
        Ok(async move { fut.await }.boxed().into())
    }

    fn handle_dump_local_gossip_data(
        &mut self,
        agent: Arc<KitsuneAgent>,
    ) -> HarnessControlApiHandlerResult<HashMap<Arc<KitsuneOpHash>, String>> {
        let (_, ctrl) = self
            .agents
            .get(&agent)
            .ok_or_else(|| KitsuneP2pError::from("invalid agent"))?;
        let fut = ctrl.dump_local_gossip_data();
        Ok(async move { fut.await }.boxed().into())
    }

    fn handle_dump_local_peer_data(
        &mut self,
        agent: Arc<KitsuneAgent>,
    ) -> HarnessControlApiHandlerResult<HashMap<Arc<KitsuneAgent>, Arc<AgentInfoSigned>>> {
        let (_, ctrl) = self
            .agents
            .get(&agent)
            .ok_or_else(|| KitsuneP2pError::from("invalid agent"))?;
        let fut = ctrl.dump_local_peer_data();
        Ok(async move { fut.await }.boxed().into())
    }
}
