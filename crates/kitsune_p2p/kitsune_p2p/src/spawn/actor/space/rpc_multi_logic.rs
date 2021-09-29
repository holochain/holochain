use super::*;
use futures::future::BoxFuture;
use kitsune_p2p_types::agent_info::AgentInfoSigned;
use kitsune_p2p_types::reverse_semaphore::*;
use kitsune_p2p_types::task_agg::TaskAgg;
use kitsune_p2p_types::tx2::tx2_utils::*;
use std::future::Future;
use tokio::sync::Notify;

pub(crate) async fn handle_rpc_multi(
    input: actor::RpcMulti,
    ro_inner: Arc<SpaceReadOnlyInner>,
    local_joined_agents: HashSet<Arc<KitsuneAgent>>,
) -> KitsuneP2pResult<Vec<actor::RpcMultiResponse>> {
    let (driver, agg) = TaskAgg::new();

    let out = Outer::new(input, ro_inner, local_joined_agents, agg);

    driver.await;

    Ok(out.finish())
}

struct Inner {
    response: Vec<actor::RpcMultiResponse>,
    remain_remote_count: u8,
    already_tried: HashSet<Arc<KitsuneAgent>>,
}

fn check_already_tried(inner: &mut Inner, agent: &Arc<KitsuneAgent>) -> bool {
    if inner.already_tried.contains(agent) {
        true
    } else {
        inner.already_tried.insert(agent.clone());
        false
    }
}

fn check_local_agent(inner: &Share<Inner>, agent: &Arc<KitsuneAgent>) -> bool {
    inner
        .share_mut(|i, _| Ok(check_already_tried(i, agent)))
        .expect("we never close this share")
}

fn check_remote_agent(inner: &Share<Inner>, agent: &Arc<KitsuneAgent>) -> bool {
    inner
        .share_mut(|i, _| {
            if i.remain_remote_count == 0 || check_already_tried(i, agent) {
                Ok(true)
            } else {
                i.remain_remote_count -= 1;
                Ok(false)
            }
        })
        .expect("we never close this share")
}

struct Outer {
    inner: Share<Inner>,
    ro_inner: Arc<SpaceReadOnlyInner>,
    agg: TaskAgg,
    kill: Arc<Kill>,
    got_data: Arc<Notify>,
    grace_rs: ReverseSemaphore,
    remote_request_grace_ms: u64,
    max_timeout: KitsuneTimeout,
    space: Arc<KitsuneSpace>,
    from_agent: Arc<KitsuneAgent>,
    basis: Arc<KitsuneBasis>,
    payload: Vec<u8>,
}
struct Kill {
    closed: AtomicBool,
    kill: Notify,
}

impl Kill {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            closed: AtomicBool::new(false),
            kill: Notify::new(),
        })
    }
    fn kill_all(&self) {
        self.closed
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.kill.notify_waiters();
    }
    async fn wait(&self) {
        if !self.closed.load(std::sync::atomic::Ordering::Relaxed) {
            self.kill.notified().await;
        }
    }
}

struct Kill {
    closed: AtomicBool,
    kill: Notify,
}

impl Kill {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            closed: AtomicBool::new(false),
            kill: Notify::new(),
        })
    }
    fn kill_all(&self) {
        self.closed
            .store(true, std::sync::atomic::Ordering::Release);
        self.kill.notify_waiters();
    }
    async fn wait(&self) {
        if !self.closed.load(std::sync::atomic::Ordering::Acquire) {
            self.kill.notified().await;
        }
    }
}

impl Outer {
    /// construct a new container for this rpc_multi logic
    fn new(
        input: actor::RpcMulti,
        ro_inner: Arc<SpaceReadOnlyInner>,
        local_joined_agents: HashSet<Arc<KitsuneAgent>>,
        agg: TaskAgg,
    ) -> Self {
        let RpcMulti {
            space,
            from_agent,
            basis,
            payload,
            max_remote_agent_count,
            max_timeout,
            remote_request_grace_ms,
        } = input;

        let grace_rs = ReverseSemaphore::new();
        let local_start_permit = grace_rs.acquire();
        let remote_start_permit = grace_rs.acquire();

        let out = Self {
            inner: Share::new(Inner {
                response: Vec::new(),
                remain_remote_count: max_remote_agent_count,
                already_tried: HashSet::new(),
            }),
            ro_inner,
            agg,
            kill: Kill::new(),
            got_data: Arc::new(Notify::new()),
            grace_rs,
            remote_request_grace_ms,
            max_timeout,
            space,
            from_agent,
            basis,
            payload,
        };

        out.add_max_timeout_task(max_timeout);
        out.add_data_and_grace_timeout_task();
        out.add_local_fetch_task(local_start_permit, local_joined_agents);
        out.add_remote_fetch_task(remote_start_permit);

        out
    }

    /// consume this logic container, returning the results
    fn finish(self) -> Vec<actor::RpcMultiResponse> {
        let Self { inner, .. } = self;

        inner
            .share_mut(|i, _| Ok(i.response.drain(..).collect()))
            .expect("we never close this share")
    }

    /// add a task that will be dropped if `kill` is notified.
    fn add_task<F>(&self, f: F)
    where
        F: Future<Output = ()> + 'static + Send,
    {
        let f = f.boxed();
        let kill = self.kill.clone();
        self.agg.push(
            async move {
                // ignore the result,
                // either we got a unit value `()` from the driver
                // or we got a notification from the kill Notify
                // either way, we don't want to process any more.
                let _ = futures::future::select(f, kill.wait().boxed()).await;
            }
            .boxed(),
        )
    }

    /// generate a closure that will
    /// add a task that will be dropped if `kill` is notified.
    ///
    /// Unlike `add_task` this task will be executed in a proper tokio task.
    /// this is needed for requests that might make calls on conductor
    /// becasue conductor has some `block_in_place` calls esp around wasm
    /// that can cause all our other pseudo-tasks managed in a single future
    /// to be cpu starved, especially around timing concerns.
    ///
    /// Once the `block_in_place` calls are removed from conductor,
    /// we can remove this specialization.
    fn gen_add_tokio_task_fn(&self) -> Arc<dyn Fn(BoxFuture<'static, ()>) + 'static + Send + Sync> {
        let kill = self.kill.clone();
        Arc::new(move |f| {
            let kill = kill.clone();
            tokio::task::spawn(async move {
                // ignore the result,
                // either we got a unit value `()` from the driver
                // or we got a notification from the kill Notify
                // either way, we don't want to process any more.
                let _ = futures::future::select(f, kill.wait().boxed()).await;
            });
        })
    }

    /// stop all processing if/when we reach our max timeout.
    fn add_max_timeout_task(&self, max_timeout: KitsuneTimeout) {
        let kill = self.kill.clone();

        self.add_task(async move {
            // wait the max timeout
            tokio::time::sleep(max_timeout.time_remaining()).await;

            // end all processing
            kill.kill_all();

            tracing::trace!("(rpc_multi_logic) max time elapsed");
        });
    }

    /// once we have any data, wait for any grace period timeouts,
    /// then allow our processing to die, and return what results we have.
    fn add_data_and_grace_timeout_task(&self) {
        let kill = self.kill.clone();
        let got_data = self.got_data.clone();
        let grace_rs = self.grace_rs.clone();

        self.add_task(async move {
            tracing::trace!("(rpc_multi_logic) grace time check start");

            // wait to have any data
            got_data.notified().await;
            tracing::trace!("(rpc_multi_logic) grace time got data");

            // wait for any pending grace permits
            grace_rs.wait_on_zero_permits().await;
            tracing::trace!("(rpc_multi_logic) grace time zero permits");

            // end all processing
            kill.kill_all();

            tracing::trace!("(rpc_multi_logic) grace time elapsed");
        });
    }

    /// generate a closure that will in-turn generate a grace permit
    fn gen_grace_permit_fn(
        &self,
    ) -> Arc<dyn Fn() -> Share<ReverseSemaphorePermit> + 'static + Send + Sync> {
        let remote_request_grace_ms = self.remote_request_grace_ms;
        let agg = self.agg.clone();
        let grace_rs = self.grace_rs.clone();
        let kill = self.kill.clone();
        Arc::new(move || {
            let permit = Share::new(grace_rs.acquire());
            let permit2 = permit.clone();
            let kill = kill.clone();

            // the permit will exist for max grace period
            agg.push(
                async move {
                    let f = tokio::time::sleep(std::time::Duration::from_millis(
                        remote_request_grace_ms,
                    ))
                    .boxed();
                    let _ = futures::future::select(f, kill.wait().boxed()).await;
                    permit2.close();
                }
                .boxed(),
            );

            // return so the permit can also be closed at the end of a call
            permit
        })
    }

    /// generate a closure that will in-turn report rpc_multi results (response)
    fn gen_report_results_fn(&self) -> Arc<dyn Fn(RpcMultiResponse) + 'static + Send + Sync> {
        let inner = self.inner.clone();
        let got_data = self.got_data.clone();
        Arc::new(move |resp| {
            // store the results in our inner data structure
            inner
                .share_mut(move |i, _| {
                    i.response.push(resp);
                    Ok(())
                })
                .expect("we never close this share");

            // notify tasks that we have received data
            got_data.notify_waiters();
        })
    }

    /// generate a closure that will in-turn make a local "call" to conductor
    fn gen_local_call_fn(
        &self,
    ) -> Arc<dyn Fn(Arc<KitsuneAgent>, Share<ReverseSemaphorePermit>) + 'static + Send + Sync> {
        let add_tokio_task = self.gen_add_tokio_task_fn();
        let report_results = self.gen_report_results_fn();
        let evt_sender = self.ro_inner.evt_sender.clone();

        let space = self.space.clone();
        let from_agent = self.from_agent.clone();
        let payload = self.payload.clone();

        Arc::new(move |to_agent, permit| {
            let report_results = report_results.clone();
            let fut = evt_sender.call(
                space.clone(),
                to_agent.clone(),
                from_agent.clone(),
                payload.clone(),
            );

            // see add_tokio_task vs add_task
            add_tokio_task(
                async move {
                    match fut.await {
                        Ok(res) => {
                            report_results(RpcMultiResponse {
                                agent: to_agent,
                                response: res,
                            });
                        }
                        Err(err) => {
                            tracing::warn!(?err, "local call error");
                        }
                    }

                    permit.close();
                }
                .boxed(),
            );
        })
    }

    /// generate a closure that will in-turn make a remote "call" to agent
    fn gen_remote_call_fn(
        &self,
    ) -> Arc<dyn Fn(AgentInfoSigned, Share<ReverseSemaphorePermit>) + 'static + Send + Sync> {
        let add_tokio_task = self.gen_add_tokio_task_fn();
        let report_results = self.gen_report_results_fn();

        let ro_inner = self.ro_inner.clone();
        let space = self.space.clone();
        let from_agent = self.from_agent.clone();
        let payload = self.payload.clone();
        let max_timeout = self.max_timeout;

        Arc::new(move |info, permit| {
            let report_results = report_results.clone();
            let ro_inner = ro_inner.clone();
            let space = space.clone();
            let from_agent = from_agent.clone();
            let payload = payload.clone();

            add_tokio_task(
                async move {
                    use discover::PeerDiscoverResult;

                    let con_hnd =
                        match discover::peer_connect(ro_inner.clone(), &info, max_timeout).await {
                            PeerDiscoverResult::OkShortcut => {
                                permit.close();
                                return;
                            }
                            PeerDiscoverResult::Err(err) => {
                                tracing::warn!(?err, "remote call error");
                                permit.close();
                                return;
                            }
                            PeerDiscoverResult::OkRemote { con_hnd, .. } => con_hnd,
                        };

                    let msg =
                        wire::Wire::call(space, from_agent, info.agent.clone(), payload.into());

                    let res = con_hnd.request(&msg, max_timeout).await;

                    match res {
                        Ok(wire::Wire::CallResp(c)) => {
                            report_results(RpcMultiResponse {
                                agent: info.agent.clone(),
                                response: c.data.into(),
                            });
                        }
                        oth => {
                            tracing::warn!(?oth, "unexpected remote call result");
                        }
                    }

                    permit.close();
                }
                .boxed(),
            );
        })
    }

    /// fetch results from any matching local agents
    fn add_local_fetch_task(
        &self,
        startup_permit: ReverseSemaphorePermit,
        local_joined_agents: HashSet<Arc<KitsuneAgent>>,
    ) {
        let inner = self.inner.clone();
        let grace_permit = self.gen_grace_permit_fn();
        let local_call = self.gen_local_call_fn();

        self.add_task(async move {
            let _startup_permit = startup_permit;

            let agent_count = local_joined_agents.len();
            tracing::trace!(%agent_count, "(rpc_multi_logic) local get start");

            for agent in local_joined_agents {
                if check_local_agent(&inner, &agent) {
                    continue;
                }

                // get a new permit for this call
                let permit = grace_permit();

                local_call(agent, permit);
            }

            tracing::trace!("(rpc_multi_logic) local get done");
        });
    }

    /// fetch results from discovered remote nodes
    fn add_remote_fetch_task(&self, startup_permit: ReverseSemaphorePermit) {
        let inner = self.inner.clone();
        let ro_inner = self.ro_inner.clone();
        let basis = self.basis.clone();
        let max_timeout = self.max_timeout;
        let grace_permit = self.gen_grace_permit_fn();
        let add_tokio_task = self.gen_add_tokio_task_fn();
        let remote_call = self.gen_remote_call_fn();

        // see add_tokio_task vs add_task
        add_tokio_task(
            async move {
                let first_discover_permit = grace_permit();
                drop(startup_permit);

                tracing::trace!("(rpc_multi_logic) remote get cached start");

                // first try remotes we already know about
                if let Ok(infos) = discover::get_cached_remotes_near_basis(
                    ro_inner.clone(),
                    basis.get_loc(),
                    max_timeout,
                )
                .await
                {
                    let cached_remote_count = infos.len();
                    tracing::trace!(
                        %cached_remote_count,
                        "(rpc_multi_logic) remote get cached",
                    );

                    for info in infos {
                        if check_remote_agent(&inner, &info.agent) {
                            continue;
                        }

                        let permit = grace_permit();
                        remote_call(info, permit);
                    }
                }

                // if we sent our request count already, we can return
                let already_done = inner
                    .share_mut(|i, _| {
                        if i.remain_remote_count == 0 {
                            Ok(true)
                        } else {
                            Ok(false)
                        }
                    })
                    .expect("we never close this share");
                if already_done {
                    tracing::trace!("(rpc_multi_logic) remote get done after cached");
                    first_discover_permit.close();
                    return;
                }

                tracing::trace!("(rpc_multi_logic) remote get searched start");

                let second_discover_permit = grace_permit();
                first_discover_permit.close();

                // if we still have requests to send, let's discover new nodes
                if let Ok(infos) = discover::search_remotes_covering_basis(
                    ro_inner.clone(),
                    basis.get_loc(),
                    max_timeout,
                )
                .await
                {
                    let searched_remote_count = infos.len();
                    tracing::trace!(
                        %searched_remote_count,
                        "(rpc_multi_logic) remote get searched",
                    );

                    for info in infos {
                        if check_remote_agent(&inner, &info.agent) {
                            continue;
                        }

                        let permit = grace_permit();
                        remote_call(info, permit);
                    }
                }

                second_discover_permit.close();

                tracing::trace!("(rpc_multi_logic) remote get done");
            }
            .boxed(),
        );
    }
}

#[cfg(test)]
#[cfg(feature = "test_utils")]
mod test;
