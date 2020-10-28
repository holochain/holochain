//! Utilities to make kitsune testing a little more sane.

use crate::{
    types::{actor::*, agent_store::*, event::*},
    *,
};
use futures::future::FutureExt;
use ghost_actor::dependencies::tracing;
use std::{collections::HashMap, sync::Arc};
use tokio::stream::StreamExt;

/// initialize tracing
pub fn init_tracing() {
    let _ = ghost_actor::dependencies::tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish(),
    );
}

/// Utility trait for test values
pub trait TestVal: Sized {
    fn test_val() -> Self;
}

/// Boilerplate shortcut for implementing TestVal on an item
#[macro_export]
macro_rules! test_val  {
    ($($item:ty => $code:block,)*) => {$(
        impl TestVal for $item { fn test_val() -> Self { $code } }
    )*};
}

/// internal helper to generate randomized kitsune data items
fn rand36<F: From<Vec<u8>>>() -> Arc<F> {
    use rand::Rng;
    let mut out = vec![0; 36];
    rand::thread_rng().fill(&mut out[..]);
    Arc::new(F::from(out))
}

// setup randomized TestVal::test_val() impls for kitsune data items
test_val! {
    Arc<KitsuneSpace> => { rand36() },
    Arc<KitsuneAgent> => { rand36() },
    Arc<KitsuneBasis> => { rand36() },
    Arc<KitsuneOpHash> => { rand36() },
}

/// a small debug representation of another type
#[derive(Clone)]
pub struct Slug(String);

impl std::fmt::Debug for Slug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

macro_rules! q_slug_from {
    ($($t:ty => |$i:ident| $c:block,)*) => {$(
        impl From<$t> for Slug {
            fn from(f: $t) -> Self {
                Slug::from(&f)
            }
        }

        impl From<&$t> for Slug {
            fn from(f: &$t) -> Self {
                let $i = f;
                Self($c)
            }
        }
    )*};
}

q_slug_from! {
    Arc<KitsuneSpace> => |s| {
        let f = format!("{:?}", s);
        format!("s{}", &f[13..25])
    },
    Arc<KitsuneAgent> => |s| {
        let f = format!("{:?}", s);
        format!("a{}", &f[13..25])
    },
}

/// an event type for an event emitted by the test suite harness
#[derive(Clone, Debug)]
pub enum HarnessEventType {
    Join {
        agent: Slug,
        space: Slug,
    },
    StoreAgentInfo {
        agent: Slug,
        agent_info: Arc<AgentInfoSigned>,
    },
}

/// an event emitted by the test suite harness
#[derive(Clone, Debug)]
pub struct HarnessEvent {
    /// the nickname of the node emitting the event
    pub nick: Arc<String>,

    /// the event type
    pub ty: HarnessEventType,
}

/// a harness event channel prioritizing use ergonomics over efficiency
/// this one struct is either sender / receiver depending on what
/// fns you invoke : ) ... clone all you like
#[derive(Clone)]
pub struct HarnessEventChannel {
    nick: Arc<String>,
    chan: tokio::sync::broadcast::Sender<HarnessEvent>,
}

impl HarnessEventChannel {
    /// constructor for a new harness event channel
    pub fn new(nick: impl AsRef<str>) -> Self {
        let (chan, mut trace_recv) = tokio::sync::broadcast::channel(10);

        // we need an active dummy recv or the sends will error
        tokio::task::spawn(async move {
            while let Some(evt) = trace_recv.next().await {
                if let Ok(evt) = evt {
                    let HarnessEvent { nick, ty } = evt;
                    const T: &str = "HARNESS_EVENT";
                    tracing::debug!(
                        %T,
                        %nick,
                        ?ty,
                    );
                }
            }
        });

        Self {
            nick: Arc::new(nick.as_ref().to_string()),
            chan,
        }
    }

    /// clone this channel, but append a nickname segment to the messages
    pub fn sub_clone(&self, sub_nick: impl AsRef<str>) -> Self {
        let mut new_nick = (*self.nick).clone();
        if !new_nick.is_empty() {
            new_nick.push_str(".");
        }
        new_nick.push_str(sub_nick.as_ref());
        Self {
            nick: Arc::new(new_nick),
            chan: self.chan.clone(),
        }
    }

    /// break off a broadcast receiver. this receiver will not get historical
    /// messages... only those that are emitted going forward
    pub fn receive(&self) -> impl tokio::stream::StreamExt {
        self.chan.subscribe()
    }

    /// publish a harness event to all receivers
    pub fn publish(&self, ty: HarnessEventType) {
        self.chan
            .send(HarnessEvent {
                nick: self.nick.clone(),
                ty,
            })
            .expect("should be able to publish");
    }
}

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

        /// Create a new agent that will connect via proxy.
        fn add_nat_agent(nick: String, proxy_url: url2::Url2) -> (
            Arc<KitsuneAgent>,
            ghost_actor::GhostSender<KitsuneP2p>,
        );
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
    init_tracing();

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

ghost_actor::ghost_chan! {
    /// The api for the test harness controller
    chan HarnessInner<KitsuneP2pError> {
        fn finish_agent(
            agent: Arc<KitsuneAgent>,
            p2p: ghost_actor::GhostSender<KitsuneP2p>,
        ) -> ();
    }
}

struct HarnessActor {
    i_s: ghost_actor::GhostSender<HarnessInner>,
    harness_chan: HarnessEventChannel,
    sub_config: TransportConfig,
    space_list: Vec<Arc<KitsuneSpace>>,
    agents: HashMap<Arc<KitsuneAgent>, ghost_actor::GhostSender<KitsuneP2p>>,
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

impl ghost_actor::GhostControlHandler for HarnessActor {}

impl ghost_actor::GhostHandler<HarnessInner> for HarnessActor {}

impl HarnessInnerHandler for HarnessActor {
    fn handle_finish_agent(
        &mut self,
        agent: Arc<KitsuneAgent>,
        p2p: ghost_actor::GhostSender<KitsuneP2p>,
    ) -> HarnessInnerHandlerResult<()> {
        self.agents.insert(agent.clone(), p2p.clone());

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
        for (agent, p2p) in self.agents.iter() {
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
            let (agent, p2p) = spawn_test_agent(sub_harness, proxy_agent_config).await?;

            i_s.finish_agent(agent.clone(), p2p.clone()).await?;

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
            let (agent, p2p) = spawn_test_agent(sub_harness, direct_agent_config).await?;

            i_s.finish_agent(agent.clone(), p2p.clone()).await?;

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
            let (agent, p2p) = spawn_test_agent(sub_harness, nat_agent_config).await?;

            i_s.finish_agent(agent.clone(), p2p.clone()).await?;

            Ok((agent, p2p))
        }
        .boxed()
        .into())
    }
}

async fn spawn_test_agent(
    harness_chan: HarnessEventChannel,
    config: KitsuneP2pConfig,
) -> Result<(Arc<KitsuneAgent>, ghost_actor::GhostSender<KitsuneP2p>), KitsuneP2pError> {
    let agent: Arc<KitsuneAgent> = TestVal::test_val();
    let (p2p, evt) = spawn_kitsune_p2p(config).await?;

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let channel_factory = builder.channel_factory().clone();

    channel_factory.attach_receiver(evt).await?;

    tokio::task::spawn(builder.spawn(AgentHarness::new(harness_chan)));

    Ok((agent, p2p))
}

struct AgentHarness {
    harness_chan: HarnessEventChannel,
    agent_store: HashMap<Arc<KitsuneAgent>, Arc<AgentInfoSigned>>,
}

impl AgentHarness {
    pub fn new(harness_chan: HarnessEventChannel) -> Self {
        Self {
            harness_chan,
            agent_store: HashMap::new(),
        }
    }
}

impl ghost_actor::GhostControlHandler for AgentHarness {}

impl ghost_actor::GhostHandler<KitsuneP2pEvent> for AgentHarness {}

impl KitsuneP2pEventHandler for AgentHarness {
    fn handle_put_agent_info_signed(
        &mut self,
        input: PutAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<()> {
        let info = Arc::new(input.agent_info_signed);
        self.agent_store.insert(input.agent.clone(), info.clone());
        self.harness_chan.publish(HarnessEventType::StoreAgentInfo {
            agent: (&input.agent).into(),
            agent_info: info,
        });
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_get_agent_info_signed(
        &mut self,
        input: GetAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        let res = self.agent_store.get(&input.agent).map(|i| (**i).clone());
        Ok(async move { Ok(res) }.boxed().into())
    }

    fn handle_call(
        &mut self,
        _space: Arc<super::KitsuneSpace>,
        _to_agent: Arc<super::KitsuneAgent>,
        _from_agent: Arc<super::KitsuneAgent>,
        _payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<Vec<u8>> {
        unimplemented!()
    }

    fn handle_notify(
        &mut self,
        _space: Arc<super::KitsuneSpace>,
        _to_agent: Arc<super::KitsuneAgent>,
        _from_agent: Arc<super::KitsuneAgent>,
        _payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        unimplemented!()
    }

    fn handle_gossip(
        &mut self,
        _space: Arc<super::KitsuneSpace>,
        _to_agent: Arc<super::KitsuneAgent>,
        _from_agent: Arc<super::KitsuneAgent>,
        _op_hash: Arc<super::KitsuneOpHash>,
        _op_data: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        unimplemented!()
    }

    fn handle_fetch_op_hashes_for_constraints(
        &mut self,
        _input: FetchOpHashesForConstraintsEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<Arc<super::KitsuneOpHash>>> {
        unimplemented!()
    }

    fn handle_fetch_op_hash_data(
        &mut self,
        _input: FetchOpHashDataEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<(Arc<super::KitsuneOpHash>, Vec<u8>)>> {
        unimplemented!()
    }

    fn handle_sign_network_data(
        &mut self,
        _input: SignNetworkDataEvt,
    ) -> KitsuneP2pEventHandlerResult<KitsuneSignature> {
        unimplemented!()
    }
}
