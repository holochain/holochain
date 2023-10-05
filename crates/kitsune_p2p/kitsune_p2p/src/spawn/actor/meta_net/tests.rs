use super::*;

use kitsune_p2p_fetch::OpHashSized;
use kitsune_p2p_timestamp::Timestamp;
use must_future::MustBoxFuture;
use std::sync::Arc;

use kitsune_p2p_types::{
    bin_types::KitsuneSpace,
    dependencies::lair_keystore_api,
    dht::{
        region::{Region, RegionCoords},
        region_set::RegionSetLtcs,
        spacetime::Topology,
    },
    dht_arc::DhtArcSet,
    metrics::MetricRecord,
    KOpData, KOpHash,
};

use crate::spawn::actor::FetchContext;
use crate::spawn::actor::FetchKey;
use crate::spawn::actor::FetchSource;
use crate::spawn::actor::MaybeDelegate;
use crate::spawn::actor::MetricExchangeMsg;
use crate::spawn::actor::OpHashList;
use crate::spawn::BroadcastData;

use crate::event::GetAgentInfoSignedEvt;
use crate::event::*;

use crate::spawn::{Internal, InternalHandler, InternalHandlerResult};

macro_rules! write_test_struct {
    ($(
        $ity:ty {
            $(
                fn $fna:ident (
                    $fself:ty,
                    $(
                        $fpna:ident: $fpty:ty,
                    )*
                ) -> $fret1:ty, $fret2:ty $fdef:block
            )*
        }
    )*) => {
        pub struct Test {
            recv: Arc<dyn Fn(MetaNetEvt) + 'static + Send + Sync>,
            $($(
                $fna: Arc<dyn Fn(
                    $(
                        $fpty,
                    )*
                ) -> $fret2 + 'static + Send + Sync>,
            )*)*
        }

        impl std::fmt::Debug for Test {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("Test").finish()
            }
        }

        impl Default for Test {
            fn default() -> Self {
                Self {
                    recv: Arc::new(|_evt: MetaNetEvt| {}),
                    $($(
                        $fna: Arc::new(|$(
                            $fpna: $fpty,
                        )*| $fdef),
                    )*)*
                }
            }
        }

        $(
            impl $ity for RunningTest {
                $(
                    fn $fna(self: $fself, $(
                        $fpna: $fpty,
                    )*) -> $fret1 {
                        (self.0.$fna)($(
                            $fpna,
                        )*).into()
                    }
                )*
            }
        )*
    };
}

type HostRes<T> = Result<T, Box<dyn Send + Sync + std::error::Error>>;
type HostRet<T> = std::pin::Pin<Box<dyn std::future::Future<Output = HostRes<T>> + 'static + Send>>;

write_test_struct! {
    KitsuneHost {
        fn block(&Self, _input: kitsune_p2p_block::Block,) -> KitsuneHostResult<()>, HostRet<()> {
            Box::pin(async move {
                Ok(())
            })
        }
        fn unblock(&Self, _input: kitsune_p2p_block::Block,) -> KitsuneHostResult<()>, HostRet<()> {
            Box::pin(async move {
                Ok(())
            })
        }
        fn is_blocked(&Self, _input: kitsune_p2p_block::BlockTargetId, _timestamp: Timestamp,) -> KitsuneHostResult<bool>, HostRet<bool> {
            Box::pin(async move {
                Ok(false)
            })
        }
        fn get_agent_info_signed(
            &Self,
            _input: GetAgentInfoSignedEvt,
        ) -> KitsuneHostResult<Option<crate::types::agent_store::AgentInfoSigned>>, HostRet<Option<crate::types::agent_store::AgentInfoSigned>> {
            Box::pin(async move {
                Ok(None)
            })
        }
        fn remove_agent_info_signed(&Self, _input: GetAgentInfoSignedEvt,) -> KitsuneHostResult<bool>, HostRet<bool> {
            Box::pin(async move {
                Ok(false)
            })
        }
        fn peer_extrapolated_coverage(
            &Self,
            _space: Arc<KitsuneSpace>,
            _dht_arc_set: DhtArcSet,
        ) -> KitsuneHostResult<Vec<f64>>, HostRet<Vec<f64>> {
            Box::pin(async move {
                Ok(vec![])
            })
        }
        fn query_region_set(
            &Self,
            _space: Arc<KitsuneSpace>,
            _dht_arc_set: Arc<DhtArcSet>,
        ) -> KitsuneHostResult<RegionSetLtcs>, HostRet<RegionSetLtcs> {
            Box::pin(async move {
                Ok(RegionSetLtcs::empty())
            })
        }
        fn query_size_limited_regions(
            &Self,
            _space: Arc<KitsuneSpace>,
            _size_limit: u32,
            _regions: Vec<Region>,
        ) -> KitsuneHostResult<Vec<Region>>, HostRet<Vec<Region>> {
            Box::pin(async move {
                Ok(vec![])
            })
        }
        fn query_op_hashes_by_region(
            &Self,
            _space: Arc<KitsuneSpace>,
            _region: RegionCoords,
        ) -> KitsuneHostResult<Vec<OpHashSized>>, HostRet<Vec<OpHashSized>> {
            Box::pin(async move {
                Ok(vec![])
            })
        }
        fn record_metrics(
            &Self,
            _space: Arc<KitsuneSpace>,
            _records: Vec<MetricRecord>,
        ) -> KitsuneHostResult<()>, HostRet<()> {
            Box::pin(async move {
                Ok(())
            })
        }
        fn get_topology(&Self, _space: Arc<KitsuneSpace>,) -> KitsuneHostResult<Topology>, HostRet<Topology> {
            Box::pin(async move {
                Ok(Topology::unit_zero())
            })
        }
        fn op_hash(&Self, _op_data: KOpData,) -> KitsuneHostResult<KOpHash>, HostRet<KOpHash> {
            Box::pin(async move {
                Ok(Arc::new(KitsuneOpHash::new(vec![0; 36])))
            })
        }
        fn check_op_data(
            &Self,
            space: Arc<KitsuneSpace>,
            op_hash_list: Vec<KOpHash>,
            _context: Option<kitsune_p2p_fetch::FetchContext>,
        ) -> KitsuneHostResult<Vec<bool>>, HostRet<Vec<bool>> {
            let _space = space;
            Box::pin(async move {
                Ok(op_hash_list.into_iter().map(|_| false).collect())
            })
        }
        fn lair_tag(&Self,) -> Option<Arc<str>>, Option<Arc<str>> {
            None
        }
        fn lair_client(&Self,) -> Option<lair_keystore_api::LairClient>, Option<lair_keystore_api::LairClient> {
            None
        }
    }
    InternalHandler {
        fn handle_register_space_event_handler(
            &mut Self,
            _recv: futures::channel::mpsc::Receiver<KitsuneP2pEvent>,
        ) -> InternalHandlerResult<()>, InternalHandlerResult<()>{
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(())
            }).into())
        }
        fn handle_incoming_delegate_broadcast(
            &mut Self,
            _space: Arc<KitsuneSpace>,
            _basis: Arc<KitsuneBasis>,
            _to_agent: Arc<KitsuneAgent>,
            _mod_idx: u32,
            _mod_cnt: u32,
            _data: BroadcastData,
        ) -> InternalHandlerResult<()>, InternalHandlerResult<()> {
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(())
            }).into())
        }
        fn handle_incoming_publish(
            &mut Self,
            _space: KSpace,
            _to_agent: KAgent,
            _source: KAgent,
            _op_hash_list: OpHashList,
            _context: kitsune_p2p_fetch::FetchContext,
            _maybe_delegate: MaybeDelegate,
        ) -> InternalHandlerResult<()>, InternalHandlerResult<()> {
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(())
            }).into())
        }
        fn handle_resolve_publish_pending_delegates(
            &mut Self,
            _space: KSpace,
            _op_hash: KOpHash,
        ) -> InternalHandlerResult<()>, InternalHandlerResult<()> {
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(())
            }).into())
        }
        fn handle_incoming_gossip(
            &mut Self,
            _space: Arc<KitsuneSpace>,
            _con: MetaNetCon,
            _remote_url: String,
            _data: Box<[u8]>,
            _module_type: GossipModuleType,
        ) -> InternalHandlerResult<()>, InternalHandlerResult<()> {
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(())
            }).into())
        }
        fn handle_incoming_metric_exchange(
            &mut Self,
            _space: Arc<KitsuneSpace>,
            _msgs: Vec<MetricExchangeMsg>,
        ) -> InternalHandlerResult<()>, InternalHandlerResult<()> {
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(())
            }).into())
        }
        fn handle_new_con(&mut Self, _url: String, _con: MetaNetCon,) -> InternalHandlerResult<()>, InternalHandlerResult<()> {
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(())
            }).into())
        }
        fn handle_del_con(&mut Self, _url: String,) -> InternalHandlerResult<()>, InternalHandlerResult<()> {
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(())
            }).into())
        }
        fn handle_fetch(
            &mut Self,
            _key: FetchKey,
            _space: KSpace,
            _source: FetchSource,
        ) -> InternalHandlerResult<()>, InternalHandlerResult<()> {
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(())
            }).into())
        }
        fn handle_get_all_local_joined_agent_infos(
            &mut Self,
        ) -> InternalHandlerResult<Vec<AgentInfoSigned>>, InternalHandlerResult<Vec<AgentInfoSigned>> {
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(vec![])
            }).into())
        }
    }
    KitsuneP2pEventHandler {
        fn handle_put_agent_info_signed(&mut Self, input: PutAgentInfoSignedEvt,) -> KitsuneP2pEventHandlerResult<()>, KitsuneP2pEventHandlerResult<()> {
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(())
            }).into())
        }
        fn handle_query_agents(&mut Self, input: QueryAgentsEvt,) -> KitsuneP2pEventHandlerResult<Vec<crate::types::agent_store::AgentInfoSigned>>, KitsuneP2pEventHandlerResult<Vec<crate::types::agent_store::AgentInfoSigned>> {
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(vec![])
            }).into())
        }
        fn handle_query_peer_density(&mut Self, space: KSpace, dht_arc: kitsune_p2p_types::dht_arc::DhtArc,) -> KitsuneP2pEventHandlerResult<kitsune_p2p_types::dht::PeerView>, KitsuneP2pEventHandlerResult<kitsune_p2p_types::dht::PeerView> {
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(kitsune_p2p_types::dht::PeerViewQ::new(
                    Topology::unit_zero(),
                    crate::dht::ArqStrat::default(),
                    vec![],
                ).into())
            }).into())
        }
        fn handle_call(&mut Self, space: KSpace, to_agent: KAgent, payload: Payload,) -> KitsuneP2pEventHandlerResult<Vec<u8>>, KitsuneP2pEventHandlerResult<Vec<u8>> {
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(vec![])
            }).into())
        }
        fn handle_notify(&mut Self, space: KSpace, to_agent: KAgent, payload: Payload,) -> KitsuneP2pEventHandlerResult<()>, KitsuneP2pEventHandlerResult<()> {
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(())
            }).into())
        }
        fn handle_receive_ops(
            &mut Self,
            space: KSpace,
            ops: Vec<KOp>,
            context: Option<FetchContext>,
        ) -> KitsuneP2pEventHandlerResult<()>, KitsuneP2pEventHandlerResult<()> {
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(())
            }).into())
        }
        fn handle_query_op_hashes(&mut Self, input: QueryOpHashesEvt,) -> KitsuneP2pEventHandlerResult<Option<(Vec<KOpHash>, TimeWindowInclusive)>>, KitsuneP2pEventHandlerResult<Option<(Vec<KOpHash>, TimeWindowInclusive)>> {
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(None)
            }).into())
        }
        fn handle_fetch_op_data(&mut Self, input: FetchOpDataEvt,) -> KitsuneP2pEventHandlerResult<Vec<(KOpHash, KOp)>>, KitsuneP2pEventHandlerResult<Vec<(KOpHash, KOp)>> {
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(vec![])
            }).into())
        }
        fn handle_sign_network_data(&mut Self, input: SignNetworkDataEvt,) -> KitsuneP2pEventHandlerResult<super::KitsuneSignature>, KitsuneP2pEventHandlerResult<super::KitsuneSignature> {
            Ok(futures::future::FutureExt::boxed(async move {
                Ok(super::KitsuneSignature(vec![0; 64]))
            }).into())
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunningTest(pub Arc<Test>);

impl ghost_actor::GhostControlHandler for RunningTest {}
impl ghost_actor::GhostHandler<Internal> for RunningTest {}
impl ghost_actor::GhostHandler<KitsuneP2pEvent> for RunningTest {}

impl RunningTest {
    fn spawn_receiver(&self, mut recv: MetaNetEvtRecv) {
        let inner = self.0.clone();
        tokio::task::spawn(async move {
            while let Some(evt) = recv.next().await {
                (inner.recv)(evt);
            }
        });
    }
}

impl Test {
    async fn spawn(
        self,
    ) -> (
        RunningTest,
        ghost_actor::GhostSender<Internal>,
        futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    ) {
        let (send, recv) = futures::channel::mpsc::channel(10);

        let test = RunningTest(Arc::new(self));
        let builder = ghost_actor::actor_builder::GhostActorBuilder::new();
        let i_s = builder.channel_factory().create_channel().await.unwrap();
        builder
            .channel_factory()
            .attach_receiver(recv)
            .await
            .unwrap();
        tokio::task::spawn(builder.spawn(test.clone()));
        (test, i_s, send)
    }
}

fn start_signal_srv() -> (std::net::SocketAddr, tokio::task::AbortHandle) {
    let mut config = tx5_signal_srv::Config::default();
    config.interfaces = "127.0.0.1".to_string();
    config.port = 0;
    config.demo = false;
    let (sig_driver, addr_list, err_list) = tx5_signal_srv::exec_tx5_signal_srv(config).unwrap();

    assert!(err_list.is_empty());
    assert_eq!(1, addr_list.len());

    let abort_handle = tokio::spawn(async move {
        sig_driver.await;
    })
    .abort_handle();

    (addr_list.first().unwrap().clone(), abort_handle)
}

#[tokio::test(flavor = "multi_thread")]
async fn meta_net_sanity() {
    let tuning_params = KitsuneP2pTuningParams::default();

    let (sig_addr, sig_abort) = start_signal_srv();

    let recv_done = Arc::new(tokio::sync::Notify::new());

    let mut test = Test::default();

    {
        let recv_done = recv_done.clone();
        test.recv = Arc::new(move |evt| {
            if let MetaNetEvt::Notify { data, .. } = evt {
                assert!(matches!(
                    data,
                    wire::Wire::Failure(wire::Failure {
                        reason,
                    }) if reason == "Hello World!",
                ));
                recv_done.notify_waiters();
            }
        });
    }

    let (test, i_s, evt_sender) = test.spawn().await;

    let (send1, recv1) = MetaNet::new_tx5(
        tuning_params.clone(),
        HostApiLegacy {
            api: Arc::new(test.clone()),
            legacy: evt_sender.clone(),
        },
        i_s.clone(),
        format!("ws://{sig_addr}"),
    )
    .await
    .unwrap();
    test.spawn_receiver(recv1);

    let (send2, recv2) = MetaNet::new_tx5(
        tuning_params.clone(),
        HostApiLegacy {
            api: Arc::new(test.clone()),
            legacy: evt_sender.clone(),
        },
        i_s.clone(),
        format!("ws://{sig_addr}"),
    )
    .await
    .unwrap();
    test.spawn_receiver(recv2);

    let addr2 = send2.local_addr().unwrap();

    let con = send1
        .get_connection(addr2, tuning_params.implicit_timeout())
        .await
        .unwrap();

    con.notify(
        &wire::Wire::failure("Hello World!".into()),
        tuning_params.implicit_timeout(),
    )
    .await
    .unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(10), recv_done.notified())
        .await
        .unwrap();

    send1.close(0, "").await;
    send2.close(0, "").await;
    sig_abort.abort();
}
