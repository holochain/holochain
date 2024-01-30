#![allow(missing_docs)] // TODO: remove this

//! This module implements value sharing for out-of-band communication between test agents.

/*
 * TODO: rewrite all the tests to use the same logic for testing all trait impls
 */

use std::{collections::BTreeMap, time::Duration};

use anyhow::Result as Fallible;
use async_trait::async_trait;
use dyn_clone::DynClone;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct AgentDummyInfo {
    id: Uuid,
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
                    eprintln!("{key} matched by {pattern}");
                    notifier.notify_waiters();
                } else {
                    eprintln!("{key} not matched by {pattern}");
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

    // TODO: this is only used to import the proc macro `SerializedBytes`. figure out how to import that selectively
    use crate::prelude::*;

    use super::local_v1::LocalV1;
    use super::{Data, SharedValues};

    pub const SHARED_VALUES_REMOTEV1_URL_ENV: &str = "TEST_SHARED_VALUES_REMOTEV1_URL";
    pub const SHARED_VALUES_REMOTEV1_URL_DEFAULT: &str = "ws://127.0.0.1:0";

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
        pub async fn new(bind_socket: Option<&str>) -> Fallible<Self> {
            let localv1 = LocalV1::default();

            let original_url =
                url2::Url2::try_parse(bind_socket.unwrap_or(SHARED_VALUES_REMOTEV1_URL_DEFAULT))?;

            let mut server = WebsocketListener::bind(
                original_url.clone(),
                std::sync::Arc::new(WebsocketConfig::default()),
            )
            .await?;

            let local_addr = server.local_addr().clone();

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

                let handle: JoinHandle<Fallible<()>> = tokio::task::spawn(async move {
                    if let Some((msg, holochain_websocket::Respond::Request(respond_fn))) =
                        recv.next().await
                    {
                        // Deserialize the message
                        let incoming_msg: RequestMessage = match msg.clone().try_into() {
                            Ok(msg) => msg,
                            Err(e) => {
                                println!("couldn't convert request {msg:?}: {e:#?}, discarding");
                                return Ok(());
                            }
                        };

                        println!("received {incoming_msg:#?}");

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

                        println!("about to send response: {response_msg:#?}");

                        let response: SerializedBytes = match response_msg.clone().try_into() {
                            Ok(msg) => msg,
                            Err(e) => {
                                println!("couldn't convert response {response_msg:?}: {e:#?}, discarding");
                                return Ok(());
                            }
                        };

                        if let Err(e) = respond_fn(response).await.map_err(anyhow::Error::from) {
                            println!("error responding: {e:#?}");
                            return Ok(());
                        }
                    }

                    Ok(())
                });

                if let Err(e) = handle.await {
                    println!("error while handling request: {e:#?}");
                }
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

        pub fn abort(self) {
            self.server_handle.abort();
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
        sender: Arc<Mutex<holochain_websocket::WebsocketSender>>,
        receiver: Arc<Mutex<holochain_websocket::WebsocketReceiver>>,
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
                sender: Arc::new(Mutex::new(sender)),
                receiver: Arc::new(Mutex::new(receiver)),
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

            let response: ResponseMessage = self
                .sender
                .clone()
                .lock()
                .await
                .clone()
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
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn shared_values_remotev1_server_message_test() {
            let server = RemoteV1Server::new(None).await.unwrap();

            let url = server.url();

            for i in 0..10 {
                // TODO: make it work while reusing the sender
                // Connect a client to the server
                let (mut send, _recv) = holochain_websocket::connect(
                    url.clone(),
                    std::sync::Arc::new(WebsocketConfig::default()),
                )
                .await
                .unwrap();

                let s = format!("expecting this back {i}");

                // Make a request and get the echoed response
                match send.request(RequestMessage::Test(s.clone())).await {
                    Ok(ResponseMessage::Test(r)) => assert_eq!(s, r),
                    other => panic!("[{i}] {other:#?}"),
                };
            }

            server.clone().abort();
            server.join().await.unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use tests::local_v1::*;
    use tests::remote_v1::*;

    use std::time::Duration;

    async fn inner_shared_values_trait_get_works(mut values: Box<dyn SharedValues>) {
        let prefix = "something".to_string();
        let s = "we expect this back".to_string();

        // TODO: remove it from here
        let prev = values.put_t(prefix.clone(), s.clone()).await.unwrap();
        println!("successfully put {prefix} = {s}; prev: {prev:#?}");

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

        // TODO: uncomment this because it should be here
        // values.put_t(prefix, s.clone()).await.unwrap();

        let got = handle.await.unwrap().unwrap();

        assert_eq!(Some(s), got);

        // {
        //     Ok(_) => (),
        //     Err(e @ tokio::task::JoinError { .. }) => {
        //         if let Ok(reason) = e.try_into_panic() {
        //             let e = format!("{reason:#?}");

        //             if let Ok(e) = reason.downcast::<anyhow::Error>() {
        //                 panic!("{e:#?}");
        //             }

        //             panic!("{e}");
        //         } else {
        //             panic!("unknown reason");
        //         }
        //     }
        // }
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

        let handle = {
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
                    eprintln!("got {got}");
                    assert_eq!(s, got);

                    got
                }
            })
        };

        // make sure the getter really comes first
        tokio::select! {
            _ = async {
                loop {
                    let num = values.num_waiters_t().await.unwrap();
                    match num {
                        0 => tokio::time::sleep(Duration::from_millis(10)).await,
                        EXPECTED_NUM_WAITERS => { eprintln!("saw a waiter!"); break },
                        _ => panic!("saw more than one waiter"),
                    };
                }
            } => { }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                panic!("didn't see a waiter");
            }
        };

        values.put_t(prefix, s).await.unwrap();

        if let Err(e) = handle.await {
            panic!("{:#?}", e);
        };
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
}
