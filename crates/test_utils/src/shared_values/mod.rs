#![allow(missing_docs)] // TODO: remove this

//! This module implements value sharing for out-of-band communication between test agents.

/*
 * TODO: rewrite all the tests to use the same logic for testing all trait impls
 *
 * TODO: test case idea by ThetaSinner
 *   1. run a network with more agents than the sharding limit
 *   2. create an entry on the DHT
 *   3. ensure that the entry reaches all agents
 */

use std::{collections::BTreeMap, time::Duration};

use anyhow::Result as Fallible;
use async_trait::async_trait;
use dyn_clone::DynClone;
use serde::{Deserialize, Serialize};
use std::iter::Iterator;
use uuid::Uuid;

pub type Data<T> = BTreeMap<String, T>;

// type WaitUntilFn = dyn Fn(&'_ Data<String>) -> BoxFuture<'_, bool>;

#[async_trait]
pub(crate) trait SharedValues: DynClone + Sync + Send {
    async fn put_t(&mut self, key: String, value: String) -> Fallible<Option<String>>;
    async fn get_pattern_t(
        &mut self,
        pattern: String,
        min_data: usize,
        maybe_wait_timeout: Option<Duration>,
    ) -> Fallible<Data<String>>;
    async fn num_waiters_t(&self) -> Fallible<usize>;
}
dyn_clone::clone_trait_object!(SharedValues);

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub(crate) struct AgentDummyInfo {
    id: Uuid,
    online: bool,
}

pub(crate) mod local_v1 {
    use anyhow::Result as Fallible;
    use std::collections::BTreeMap;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use super::*;

    /// Local implementation using a guarded BTreeMap as its datastore.
    #[derive(Clone, Default)]
    pub struct LocalV1 {
        num_waiters: Arc<AtomicUsize>,
        data: Arc<tokio::sync::Mutex<BTreeMap<String, String>>>,
        notification: Arc<tokio::sync::Mutex<BTreeMap<String, Arc<tokio::sync::Notify>>>>,
    }

    impl LocalV1 {
        /// Gets all values that have a matching key prefix; waits for `min_results` to become available if specified.
        /// `wait_until` lets the caller decide under which conditions to accept the result, or otherwise keep waiting.
        ///
        /// Please look at the tests for usage examples.
        pub async fn get_pattern<F>(
            &mut self,
            pattern: &str,
            mut wait_until: F,
        ) -> Fallible<Data<String>>
        where
            F: FnMut((&Data<String>, &Data<String>)) -> bool,
        {
            // we *are* using this but not in all circumstances
            #[allow(unused_assignments)]
            let mut previous_results: Data<String> = Default::default();

            loop {
                let (notifier, notification);

                // new scope so data_guard gets dropped before waiting for a notification
                {
                    let data_guard = self.data.lock().await;

                    let mut results: Data<String> = Default::default();

                    for (key, value) in data_guard.iter() {
                        if key.matches(pattern).count() > 0 {
                            results.insert(key.to_string(), value.clone());
                        }
                    }

                    previous_results = results.clone();

                    if wait_until((&previous_results, &results)) {
                        return Ok(results);
                    }

                    // get the notifier and start waiting on it while still holding the data_guard.
                    // this prevents a race between getting the notifier and a writer just writing something and sending notifications for it
                    self.num_waiters.fetch_add(1, Ordering::SeqCst);
                    notifier = self
                        .notification
                        .lock()
                        .await
                        .entry(pattern.to_string())
                        .or_default()
                        .clone();

                    notification = notifier.notified();
                };

                notification.await;

                self.num_waiters.fetch_sub(1, Ordering::SeqCst);
            }
        }

        pub fn num_waiters(&self) -> usize {
            self.num_waiters.load(Ordering::SeqCst)
        }

        /// Puts the `value` for `key` and notifies any waiters if there are any.
        pub async fn put(&mut self, key: String, value: String) -> Fallible<Option<String>> {
            let mut data_guard = self.data.lock().await;

            let maybe_previous = if let Some(previous) = data_guard.insert(key.clone(), value) {
                Some(previous)
            } else {
                None
            };

            for (pattern, notifier) in self.notification.lock().await.iter() {
                if key.matches(pattern).count() > 0 {
                    tracing::debug!("{key} matched by {pattern}");
                    notifier.notify_waiters();
                } else {
                    tracing::debug!("{key} not matched by {pattern}");
                }
            }

            Ok(maybe_previous)
        }
    }

    #[async_trait]
    impl SharedValues for LocalV1 {
        async fn put_t(&mut self, key: String, value: String) -> Fallible<Option<String>> {
            self.put(key, value).await
        }

        async fn get_pattern_t(
            &mut self,
            pattern: String,
            min_data: usize,
            maybe_wait_timeout: Option<Duration>,
        ) -> Fallible<Data<String>> {
            tokio::select! {
                data = self.get_pattern(pattern.as_str(), |(_previous_data, data)| { data.len() >= min_data }) => Ok(data?),
                _ = {
                    let duration = if let Some(wait_timeout) =  maybe_wait_timeout {
                        wait_timeout
                    } else {
                        std::time::Duration::MAX
                    };

                    tokio::time::sleep(duration)
                }  => {
                    anyhow::bail!("timeout")
                }
            }
        }
        async fn num_waiters_t(&self) -> Fallible<usize> {
            Ok(self.num_waiters())
        }
    }
}

pub(crate) mod remote_v1 {
    use anyhow::{bail, Context, Result as Fallible};
    use async_trait::async_trait;
    use futures::StreamExt;

    use serde::{Deserialize, Serialize};
    use std::borrow::Borrow;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::task::JoinHandle;

    use holochain_websocket::{WebsocketConfig, WebsocketListener};

    // TODO: deglob this
    use holochain_serialized_bytes::prelude::*;

    use super::local_v1::LocalV1;
    use super::{Data, SharedValues};

    pub const SHARED_VALUES_REMOTEV1_URL_ENV: &str = "TEST_SHARED_VALUES_REMOTEV1_URL";
    pub const SHARED_VALUES_REMOTEV1_URL_DEFAULT: &str = "ws://127.0.0.1:0";

    // The value given to this env var is used to construct `RemoteV1Role`
    pub const SHARED_VALUES_REMOTEV1_ROLE_ENV: &str = "TEST_SHARED_VALUES_REMOTEV1_ROLE";

    #[derive(Debug, Default, strum_macros::EnumString)]
    #[strum(serialize_all = "lowercase")]
    pub(crate) enum RemoteV1Role {
        #[default]
        Both,

        Server,
        Client,
    }

    /// Remote implementation using Websockets for data passing.
    #[derive(Clone)]
    pub struct RemoteV1Server {
        local_addr: url2::Url2,

        server_handle: Arc<JoinHandle<Fallible<()>>>,
    }

    #[derive(Serialize, Deserialize, SerializedBytes, Debug, Clone)]
    pub enum RequestMessage {
        Test(String),
        Put {
            key: String,
            value: String,
        },
        Get {
            pattern: String,
            min_data: usize,
            maybe_timeout: Option<Duration>,
        },
        NumWaiters,
    }

    #[derive(Serialize, Deserialize, SerializedBytes, Debug, Clone)]
    pub enum ResponseMessage {
        StringErr(String),
        Test(String),
        Put(Result<Option<String>, String>),
        Get(Result<Data<String>, String>),
        NumWaiters(usize),
    }

    impl RemoteV1Server {
        /// Creates a new server and starts it immediately.
        pub async fn new(bind_socket: Option<String>) -> Fallible<Self> {
            let localv1 = LocalV1::default();

            let original_url = url2::Url2::try_parse(bind_socket.unwrap_or_else(|| {
                std::env::var(SHARED_VALUES_REMOTEV1_URL_ENV)
                    .map_err(|e| {
                        tracing::debug!(
                            "could not read env var {SHARED_VALUES_REMOTEV1_URL_ENV}: {e}"
                        );
                        e
                    })
                    .ok()
                    .unwrap_or(SHARED_VALUES_REMOTEV1_URL_DEFAULT.to_string())
            }))?;

            tracing::debug!("binding server to {original_url}");

            let mut server = WebsocketListener::bind(
                original_url.clone(),
                std::sync::Arc::new(WebsocketConfig::default()),
            )
            .await?;

            let local_addr = server.local_addr().clone();

            tracing::info!("server bound to {local_addr}");

            let server_handle = tokio::task::spawn(async move {
                // Handle new connections
                Self::remotev1server_inner(localv1, &mut server).await?;

                Ok(())
            });

            Ok(Self {
                local_addr,
                server_handle: Arc::new(server_handle),
            })
        }

        async fn remotev1server_inner(
            localv1: LocalV1,
            server: &mut WebsocketListener,
        ) -> Fallible<()> {
            while let Some(Ok((/* never sends on its own */ _tx, mut recv))) = server.next().await {
                let mut localv1 = localv1.clone();

                // TODO: should we join the handle in a separate task to deal with any errors?
                let _handle: JoinHandle<Fallible<()>> = tokio::task::spawn(async move {
                    if let Some((msg, holochain_websocket::Respond::Request(respond_fn))) =
                        recv.next().await
                    {
                        // Deserialize the message
                        let incoming_msg: RequestMessage = match msg.clone().try_into() {
                            Ok(msg) => msg,
                            Err(e) => {
                                tracing::warn!(
                                    "couldn't convert request {msg:?}: {e:#?}, discarding"
                                );
                                return Ok(());
                            }
                        };

                        tracing::trace!("received {incoming_msg:#?}");

                        let response_msg: ResponseMessage = match incoming_msg {
                            RequestMessage::Test(s) => ResponseMessage::Test(format!("{}", s)),

                            RequestMessage::Put { key, value } => ResponseMessage::Put(
                                localv1.put(key, value).await.map_err(|e| e.to_string()),
                            ),
                            RequestMessage::Get {
                                pattern,
                                min_data,
                                maybe_timeout,
                            } => {
                                match localv1
                                    .get_pattern_t(pattern, min_data, maybe_timeout)
                                    .await
                                {
                                    Ok(data) => ResponseMessage::Get(Ok(data)),
                                    Err(e) => ResponseMessage::StringErr(e.to_string()),
                                }
                            }

                            RequestMessage::NumWaiters => {
                                ResponseMessage::NumWaiters(localv1.num_waiters())
                            }
                        };

                        tracing::trace!("about to send response: {response_msg:#?}");

                        let response: SerializedBytes = match response_msg.clone().try_into() {
                            Ok(msg) => msg,
                            Err(e) => {
                                tracing::warn!("couldn't convert response {response_msg:?}: {e:#?}, discarding");
                                return Ok(());
                            }
                        };

                        if let Err(e) = respond_fn(response).await.map_err(anyhow::Error::from) {
                            tracing::debug!("error responding: {e:#?}");
                            return Ok(());
                        }
                    }

                    Ok(())
                });
            }

            Ok(())
        }

        pub async fn join(self) -> Fallible<()> {
            match Arc::into_inner(self.server_handle)
                .ok_or_else(|| anyhow::anyhow!("couldn't get join handle"))?
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    if e.is_cancelled() {
                        Ok(())
                    } else {
                        bail!(e)
                    }
                }
            }
        }

        pub async fn abort_and_join(self) -> Fallible<()> {
            self.server_handle.abort();
            self.join().await
        }

        pub fn url(&self) -> &url2::Url2 {
            &self.local_addr
        }
    }

    use futures::lock::Mutex;

    /// Remote implementation using Websockets for data passing.
    #[derive(Clone)]
    pub struct RemoteV1Client {
        url: url2::Url2,
        // TODO: figure out how to reuse an existing sender. the first attempt yielded errors for subsequent requests
        // sender: Arc<Mutex<holochain_websocket::WebsocketSender>>,
        // receiver: Arc<Mutex<holochain_websocket::WebsocketReceiver>>,
        maybe_request_timeout: Option<Duration>,
    }

    pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 10;

    impl RemoteV1Client {
        /// Returns a new client.
        pub async fn new(
            url: &url2::Url2,
            maybe_websocket_config: Option<WebsocketConfig>,
        ) -> Fallible<Self> {
            let (sender, receiver) = holochain_websocket::connect(
                url.clone(),
                Arc::new(maybe_websocket_config.unwrap_or_default()),
            )
            .await?;

            Ok(Self {
                url: url.clone(),
                // sender: Arc::new(Mutex::new(sender)),
                // receiver: Arc::new(Mutex::new(receiver)),
                maybe_request_timeout: None,
            })
        }

        /// Sends a request to the connected server.
        pub async fn request(
            &self,
            request: RequestMessage,
            maybe_timeout: Option<Duration>,
        ) -> Fallible<ResponseMessage> {
            let timeout = maybe_timeout.unwrap_or(
                self.maybe_request_timeout
                    .unwrap_or(std::time::Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS)),
            );

            let (mut send, _recv) = holochain_websocket::connect(
                self.url.clone(),
                std::sync::Arc::new(WebsocketConfig::default()),
            )
            .await
            .context(format!("connecting to {}", self.url))?;

            let response: ResponseMessage = send
                .request_timeout(&request, timeout)
                .await
                .context(format!("requesting {request:#?} with {timeout:?} timeout"))?;

            Ok(response)
        }
    }

    #[async_trait]
    impl SharedValues for RemoteV1Client {
        async fn put_t(&mut self, key: String, value: String) -> Fallible<Option<String>> {
            match self
                .request(
                    RequestMessage::Put { key, value },
                    self.maybe_request_timeout,
                )
                .await?
            {
                ResponseMessage::Put(result) => result.map_err(|s| anyhow::anyhow!(s)),
                other => bail!("got wrong response type {other:#?}"),
            }
        }

        async fn get_pattern_t(
            &mut self,
            pattern: String,
            min_data: usize,
            maybe_timeout: Option<Duration>,
        ) -> Fallible<Data<String>> {
            match self
                .request(
                    RequestMessage::Get {
                        pattern,
                        min_data,
                        maybe_timeout,
                    },
                    maybe_timeout,
                )
                .await
                .context("sending get request")?
            {
                ResponseMessage::Get(Ok(data)) => {
                    Ok(data)
                    // todo!("parse {data:#?}");
                }

                ResponseMessage::Get(Err(msg)) => {
                    bail!("error response: {msg}")
                }

                unexpected => {
                    bail!("unexpected response: {unexpected:#?}")
                }
            }
        }

        async fn num_waiters_t(&self) -> Fallible<usize> {
            match self.request(RequestMessage::NumWaiters, None).await? {
                ResponseMessage::NumWaiters(num_waiters) => Ok(num_waiters),

                unexpected => {
                    bail!("unexpected response: {unexpected:#?}")
                }
            }
        }
    }
    #[cfg(test)]
    mod tests {
        use std::str::FromStr;

        use url2::Url2;

        use super::*;

        // FIXME: this is racy as something else could bind the port in between
        // dropping this listener and making use of the url
        fn get_unused_ws_url() -> Fallible<String> {
            let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
            let addr_bound = listener.local_addr()?;
            Ok(format!("ws://{addr_bound}/"))
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn shared_values_remotev1_server_url_passing_fnarg() {
            let url_to_use = get_unused_ws_url().unwrap();

            let server = RemoteV1Server::new(Some(url_to_use.clone())).await.unwrap();

            assert_eq!(server.url().to_string(), url_to_use);

            server.abort_and_join().await.unwrap();
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn shared_values_remotev1_server_url_passing_env_var() {
            let url_to_use = get_unused_ws_url().unwrap();

            std::env::set_var(SHARED_VALUES_REMOTEV1_URL_ENV, url_to_use.clone());
            let server = RemoteV1Server::new(None).await.unwrap();
            assert_eq!(server.url().to_string(), url_to_use);

            server.abort_and_join().await.unwrap();
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn shared_values_remotev1_distributed() {
            let role = std::env::var(SHARED_VALUES_REMOTEV1_ROLE_ENV)
                .map(|role_env| RemoteV1Role::from_str(&role_env))
                .unwrap_or_else(|_| Ok(RemoteV1Role::default()))
                .unwrap();

            println!("starting with role: {role:#?}");

            match role {
                RemoteV1Role::Both => {
                    let server = RemoteV1Server::new(None).await.unwrap();
                    println!("server listening on {}", server.url());

                    let url_to_use = server.url();

                    echo_msg("distributed", Url2::parse(url_to_use))
                        .await
                        .unwrap();
                }
                RemoteV1Role::Server => {
                    let server = RemoteV1Server::new(None).await.unwrap();
                    println!("server listening on {}", server.url());

                    server.join().await.unwrap();
                }

                RemoteV1Role::Client => {
                    let url_to_use = std::env::var(SHARED_VALUES_REMOTEV1_URL_ENV).unwrap();
                    echo_msg("distributed", Url2::parse(url_to_use))
                        .await
                        .unwrap();
                }
            };
        }

        async fn echo_msg(i: impl std::fmt::Display, server_url: url2::Url2) -> Fallible<()> {
            // TODO: make reusing the same sender work
            // Connect a client to the server
            let (mut send, _recv) = holochain_websocket::connect(
                server_url.clone(),
                std::sync::Arc::new(WebsocketConfig::default()),
            )
            .await
            .context(format!("connecting to {server_url}"))?;

            let s = format!("expecting this back {i}");

            // Make a request and get the echoed response
            match send.request(RequestMessage::Test(s.clone())).await {
                Ok(ResponseMessage::Test(r)) => assert_eq!(s, r),
                other => panic!("request {i}: {other:#?}"),
            };

            Ok(())
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 10)]
        #[cfg(feature = "slow_tests")]
        async fn shared_values_remotev1_server_message_test() {
            const NUM_MESSAGES: usize = 5_000;

            let server = RemoteV1Server::new(None).await.unwrap();
            let server_url = server.url().clone();

            let (tasks_tx, mut tasks_rx) = tokio::sync::mpsc::channel::<JoinHandle<_>>(100);

            {
                tokio::spawn(async move {
                    for i in 0..NUM_MESSAGES {
                        let server_url = server_url.clone();
                        let sender_task =
                            tokio::spawn(
                                async move { echo_msg(i, server_url.clone()).await.unwrap() },
                            );

                        tasks_tx.send(sender_task).await.unwrap();
                    }
                });
            };

            let mut handled_tasks = 0;
            while let Some(recv) = tokio::select! {
                maybe_recv = tasks_rx.recv() => maybe_recv,

                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                    println!("did not receive a task in 100ms");
                    None
                }
            } {
                handled_tasks += 1;
                recv.await.unwrap();
            }

            assert_eq!(NUM_MESSAGES, handled_tasks);

            server.abort_and_join().await.unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::bail;
    use anyhow::Context;
    use tests::local_v1::*;
    use tests::remote_v1::*;
    use url2::Url2;

    use std::str::FromStr;
    use std::thread::JoinHandle;
    use std::time::Duration;

    async fn inner_shared_values_trait_put_works(mut values: Box<dyn SharedValues>) {
        let prefix = "something".to_string();
        let s = "we expect this back".to_string();

        let prev = values.put_t(prefix.clone(), s.clone()).await.unwrap();
        println!("successfully put {prefix} = {s}; prev: {prev:#?}");

        assert_eq!(None, prev);

        let prev = values.put_t(prefix.clone(), s.clone()).await.unwrap();
        println!("successfully put {prefix} = {s}; prev: {prev:#?}");

        assert_eq!(Some(s), prev);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shared_values_localv1_put_works() {
        inner_shared_values_trait_put_works(Box::new(LocalV1::default()) as Box<dyn SharedValues>)
            .await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shared_values_remotev1_put_works() {
        let server = RemoteV1Server::new(None).await.unwrap();

        inner_shared_values_trait_put_works(Box::new(
            RemoteV1Client::new(server.url(), None).await.unwrap(),
        ) as Box<dyn SharedValues>)
        .await;
    }

    async fn inner_shared_values_trait_get_works(mut values: Box<dyn SharedValues>) {
        let prefix = "something".to_string();
        let s = "we expect this back".to_string();

        let handle = {
            let prefix = prefix.clone();
            let mut values = values.clone();

            tokio::spawn({
                async move {
                    let got = values
                        .get_pattern_t(prefix.clone(), 0, Some(std::time::Duration::from_secs(1)))
                        .await
                        .context("call to get_pattern_t")?
                        .into_values()
                        .nth(0);

                    eprintln!("got {got:#?}");

                    Fallible::<Option<String>>::Ok(got)
                }
            })
        };

        let prev = values.put_t(prefix.clone(), s.clone()).await.unwrap();
        println!("successfully put {prefix} = {s}; prev: {prev:#?}");

        let got = handle.await.unwrap().unwrap();

        assert_eq!(Some(s), got);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shared_values_localv1_get_works() {
        inner_shared_values_trait_get_works(Box::new(LocalV1::default()) as Box<dyn SharedValues>)
            .await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shared_values_remotev1_get_works() {
        let server = RemoteV1Server::new(None).await.unwrap();

        inner_shared_values_trait_get_works(Box::new(
            RemoteV1Client::new(server.url(), None).await.unwrap(),
        ) as Box<dyn SharedValues>)
        .await;
    }

    async fn inner_shared_values_trait_get_waits(mut values: Box<dyn SharedValues>) {
        const EXPECTED_NUM_WAITERS: usize = 1;

        let prefix = "something".to_string();
        let s = "we expect this back".to_string();

        let get_handle = {
            let prefix = prefix.clone();
            let s = s.clone();
            let mut values = values.clone();

            tokio::spawn({
                async move {
                    let got: String = values
                        .get_pattern_t(prefix.clone(), EXPECTED_NUM_WAITERS, None)
                        .await
                        .unwrap()
                        .into_values()
                        .nth(0)
                        .unwrap();
                    println!("got {got}");
                    assert_eq!(s, got);

                    got
                }
            })
        };

        // make sure the getter really comes first
        tokio::select! {
            _ = async {
                for i in 0..core::usize::MAX {
                    println!("{i}: asking for num_waiters");
                    let num = values.num_waiters_t().await.unwrap();
                    match num {
                        0 => {
                            println!("{i}: still no waiter...");
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                            ,
                        EXPECTED_NUM_WAITERS => {
                            println!("{i} saw a waiter!");
                            break
                        },
                        more => panic!("{i} saw more than one waiter: {more}"),
                    };
                }
            } => { }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                panic!("didn't see a waiter");
            }
        };

        values.put_t(prefix, s).await.unwrap();

        get_handle.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shared_values_localv1_get_waits() {
        inner_shared_values_trait_get_waits(Box::new(LocalV1::default()) as Box<dyn SharedValues>)
            .await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shared_values_remotev1_get_waits() {
        let server = RemoteV1Server::new(None).await.unwrap();

        inner_shared_values_trait_get_waits(Box::new(
            RemoteV1Client::new(server.url(), None).await.unwrap(),
        ) as Box<dyn SharedValues>)
        .await;
    }

    async fn inner_shared_values_trait_simulate_agent_discovery(
        values: Box<dyn SharedValues + Sync>,
    ) {
        const PREFIX: &str = "agent_";

        let required_agents = 2;
        let num_agents = 2;

        let get_handle = {
            let mut values = values.clone();
            tokio::spawn(async move {
                tokio::select! {
                    _ = async {
                        let all_agents: Data<AgentDummyInfo> = values.get_pattern_t(PREFIX.to_string(), required_agents, None)
                            .await
                            .unwrap()
                            .into_iter()
                            .map(|(key, value)| Ok((key, serde_json::from_str(&value)?)))
                            .collect::<Fallible<_>>()
                            .unwrap();
                        assert!(required_agents <= all_agents.len());
                        assert!(all_agents.len() <= num_agents);
                        eprintln!("{} agents {all_agents:#?}", all_agents.len());
                    } => { }
                    _ = tokio::time::sleep(Duration::from_millis(50)) => { panic!("not enough agents"); }
                }
            })
        };

        let mut handles = vec![get_handle];
        for _ in 0..num_agents {
            let mut values = values.clone();

            let handle = tokio::spawn(async move {
                let agent_dummy_info = AgentDummyInfo {
                    id: uuid::Uuid::new_v4(),
                    ..Default::default()
                };
                values
                    .put_t(
                        format!("{PREFIX}{}", &agent_dummy_info.id),
                        serde_json::to_string(&agent_dummy_info).unwrap(),
                    )
                    .await
                    .unwrap();
            });
            handles.push(handle);
        }

        for handle in handles {
            if let Err(e) = handle.await {
                panic!("{:#?}", e);
            };
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn shared_values_localv1_simulate_agent_discovery() {
        let values: Box<dyn SharedValues + Sync> = Box::new(LocalV1::default());
        inner_shared_values_trait_simulate_agent_discovery(values).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn shared_values_remotev1_simulate_agent_discovery() {
        let server = RemoteV1Server::new(None).await.unwrap();

        let values: Box<dyn SharedValues + Sync> =
            Box::new(RemoteV1Client::new(server.url(), None).await.unwrap());
        inner_shared_values_trait_simulate_agent_discovery(values).await;
    }

    fn shared_values_remotev1_simulate_agent_discovery_distributed_client(
        mut values: Box<dyn SharedValues + Sync>,
        required_agents: usize,
        prefix: &'static str,
        timeout: std::time::Duration,
    ) -> tokio::task::JoinHandle<Fallible<AgentDummyInfo>> {
        tokio::spawn(async move {
            let agent_dummy_info = AgentDummyInfo {
                id: uuid::Uuid::new_v4(),
                ..Default::default()
            };

            // register this client
            values
                .put_t(
                    format!("{prefix}{}", &agent_dummy_info.id),
                    serde_json::to_string(&agent_dummy_info).unwrap(),
                )
                .await?;

            let mut all_agents: Data<AgentDummyInfo> = Default::default();
            let mut num_agents = 0;

            // wait for enough other clients to show up
            tokio::select! {
                result = async {
                    while num_agents < required_agents {
                        all_agents = values.get_pattern_t(prefix.to_string(), required_agents, None)
                            .await.context(format!("getting all agents via pattern {prefix}")).unwrap()
                            .into_iter()
                            .map(|(key, value)| Ok((key.clone(), serde_json::from_str(&value).context(format!("deserializing value for {key}: {value:#?}"))?)))
                            .collect::<Fallible<_>>().unwrap();

                        num_agents =  all_agents.len();

                        println!("{} agents {all_agents:#?}", num_agents);

                        tokio::time::sleep(Duration::from_millis(100)).await;
                    };

                    Ok(agent_dummy_info)
                } => result,
                _ = tokio::time::sleep(Duration::from_secs(10)) => {
                    bail!("not enough agents: {}", num_agents);
                }
            }
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn shared_values_remotev1_simulate_agent_discovery_distributed() {
        let role = std::env::var(SHARED_VALUES_REMOTEV1_ROLE_ENV)
            .map(|role_env| RemoteV1Role::from_str(&role_env))
            .unwrap_or_else(|_| Ok(RemoteV1Role::default()))
            .unwrap();

        println!("starting with role: {role:#?}");

        const PREFIX: &str = "shared_values_trait_simulate_agent_discovery_distributed_agent_";
        const REQUIRED_AGENTS: usize = 3;

        let mut handles: Vec<tokio::task::JoinHandle<Fallible<_>>> = Vec::new();

        match role {
            RemoteV1Role::Both => {
                let server = RemoteV1Server::new(None).await.unwrap();
                println!("server listening on {}", server.url());

                let url_to_use = server.url();

                let values: Box<dyn SharedValues + Sync> = Box::new(
                    RemoteV1Client::new(&url_to_use, None)
                        .await
                        .expect("connecting remotev1 client"),
                );

                for _ in 0..REQUIRED_AGENTS {
                    let handle = shared_values_remotev1_simulate_agent_discovery_distributed_client(
                        values.clone(),
                        REQUIRED_AGENTS,
                        PREFIX,
                    );

                    handles.push(handle);
                }

                assert_eq!(handles.len(), REQUIRED_AGENTS);

                for handle in handles {
                    // consider client errors, this could be a timeout while waiting for all agents or something else
                    // not sure why this needs double-unwrapping
                    let _result = handle.await.unwrap().unwrap();
                }
            }
            RemoteV1Role::Server => {
                let server = RemoteV1Server::new(None).await.unwrap();
                println!("server listening on {}", server.url());

                server.join().await.unwrap();
            }

            RemoteV1Role::Client => {
                let url_to_use =
                    Url2::parse(std::env::var(SHARED_VALUES_REMOTEV1_URL_ENV).unwrap());

                let values: Box<dyn SharedValues + Sync> = Box::new(
                    RemoteV1Client::new(&url_to_use, None)
                        .await
                        .expect("connecting remotev1 client"),
                );

                let _result = shared_values_remotev1_simulate_agent_discovery_distributed_client(
                    values.clone(),
                    REQUIRED_AGENTS,
                    PREFIX,
                )
                .await
                .unwrap()
                .unwrap();
            }
        };
    }
}
