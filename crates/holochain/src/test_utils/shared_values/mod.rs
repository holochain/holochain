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
    fn num_waiters_t(&self) -> Fallible<usize>;
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
        fn num_waiters_t(&self) -> Fallible<usize> {
            Ok(self.num_waiters())
        }
    }

    #[cfg(test)]
    mod tests {
        
        use std::time::Duration;
        

        use super::super::*;
        use super::*;

        #[tokio::test]
        // #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn shared_values_localv1_get_waits() {
            let mut values = Box::new(LocalV1::default()) as Box<dyn SharedValues>;

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
                        let num = values.num_waiters_t().unwrap();
                        match num {
                            0 => tokio::time::sleep(Duration::from_millis(10)).await,
                            EXPECTED_NUM_WAITERS => { eprintln!("saw a getter!"); break },
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

        #[tokio::test(flavor = "multi_thread")]
        async fn shared_values_localv1_simulate_agent_discovery() {
            let values: Box<dyn SharedValues + Sync> = Box::new(LocalV1::default());

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
    }
}

pub(crate) mod remote_v1 {
    use anyhow::{bail, Result as Fallible};
    use async_trait::async_trait;
    use futures::StreamExt;
    use serde::{Deserialize, Serialize};
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

        server_handle: Arc<JoinHandle<()>>,
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
    }

    #[derive(Serialize, Deserialize, SerializedBytes, Debug, Clone)]
    pub enum ResponseMessage {
        Test(String),
        Put(Result<Option<String>, String>),
        Get(Result<Data<String>, String>),
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
                // Handle new connections, currently doesn't propagate errors
                Self::remotev1server_inner(localv1, &mut server).await
            });

            Ok(Self {
                local_addr,
                server_handle: Arc::new(server_handle),
            })
        }

        async fn remotev1server_inner(localv1: LocalV1, server: &mut WebsocketListener) {
            while let Some(Ok((/* never sends on its own */ _, mut recv))) = server.next().await {
                let mut localv1 = localv1.clone();

                // TODO: do we need the output for anything?
                tokio::task::spawn(async move {
                    // Receive a message and echo it back
                    if let Some((msg, holochain_websocket::Respond::Request(respond_fn))) =
                        recv.next().await
                    {
                        // Deserialize the message
                        let incoming_msg: RequestMessage = match msg.clone().try_into() {
                            Ok(msg) => msg,
                            Err(e) => {
                                tracing::error!(
                                    "couldn't convert request {msg:?}: {e:#?}, discarding"
                                );
                                return;
                            }
                        };

                        let response_msg: ResponseMessage = match incoming_msg {
                            RequestMessage::Test(s) => ResponseMessage::Test(format!("{}", s)),

                            RequestMessage::Put { key, value } => ResponseMessage::Put(
                                localv1.put(key, value).await.map_err(|e| e.to_string()),
                            ),
                            RequestMessage::Get {
                                pattern: _,
                                min_data: _,
                                maybe_timeout: _,
                            } => {
                                // TODO
                                let data = Default::default();

                                ResponseMessage::Get(Ok(data))
                            }
                        };

                        let response: SerializedBytes = match response_msg.clone().try_into() {
                            Ok(msg) => msg,
                            Err(e) => {
                                tracing::error!(
                                    "couldn't convert response {response_msg:?}: {e:#?}, discarding"
                                );
                                return;
                            }
                        };

                        if let Err(e) = respond_fn(response).await.map_err(anyhow::Error::from) {
                            tracing::error!("{e}");
                        }
                    };
                });
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
    }

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
            })
        }

        /// Sends a request to the connected server.
        pub async fn request(&self, request: RequestMessage) -> Fallible<ResponseMessage> {
            let response: ResponseMessage = self
                .sender
                .lock()
                .await
                .request_timeout(request, std::time::Duration::from_secs(10))
                .await?;

            Ok(response)
        }
    }

    #[async_trait]
    impl SharedValues for RemoteV1Client {
        async fn put_t(&mut self, key: String, value: String) -> Fallible<Option<String>> {
            match self.request(RequestMessage::Put { key, value }).await? {
                ResponseMessage::Put(result) => result.map_err(|s| anyhow::anyhow!(s)),
                other => bail!("got wrong response type {other:#?}"),
            }
        }

        async fn get_pattern_t(
            &mut self,
            _pattern: String,
            _min_data: usize,
            _maybe_wait_timeout: Option<Duration>,
        ) -> Fallible<Data<String>> {
            todo!();

            // tokio::select! {
            //     data = self.get_pattern(pattern.as_str(), |(_previous_data, data)| { data.len() >= min_data }) => Ok(data?),
            //     _ = {
            //         let duration = if let Some(wait_timeout) =  maybe_wait_timeout {
            //             wait_timeout
            //         } else {
            //             std::time::Duration::MAX
            //         };

            //         tokio::time::sleep(duration)
            //     }  => todo!(),
        }
        //         anyhow::bail!("timeout")
        //     }
        // }

        fn num_waiters_t(&self) -> Fallible<usize> {
            todo!()
        }
    }
    #[cfg(test)]
    mod tests {
        

        

        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn shared_values_remotev1_server_message_test() {
            let server = RemoteV1Server::new(None).await.unwrap();

            let url = server.url();

            // Connect a client to the server
            let (mut send, _recv) = holochain_websocket::connect(
                url.clone(),
                std::sync::Arc::new(WebsocketConfig::default()),
            )
            .await
            .unwrap();

            let s = "expecting this back".to_string();

            // Make a request and get the echoed response
            match send.request(RequestMessage::Test(s.clone())).await {
                Ok(ResponseMessage::Test(r)) => assert_eq!(s, r),
                other => panic!("{other:#?}"),
            };

            server.abort();
        }

        #[tokio::test]
        async fn shared_values_remote_v1_basic() {
            let server = RemoteV1Server::new(None).await.unwrap();

            let client = RemoteV1Client::new(server.url(), None).await.unwrap();

            let s = "expecting this back".to_string();

            let r = match client.request(RequestMessage::Test(s.clone())).await {
                Ok(ResponseMessage::Test(r)) => r,
                other => panic!("{other:#?}"),
            };

            assert_eq!(s, r);
        }

        // #[tokio::test(flavor = "multi_thread")]
        // async fn shared_values_remotev1_simulate_agent_discovery() {
        //     const PREFIX: &str = "agent_";

        //     let required_agents = 2;
        //     let num_agents_required = 2;
        //     let num_agents_spawned = num_agents_required * 10;

        //     let server = RemoteV1Server::new(None).await.unwrap();
        //     let url = server.url();

        //     let agent_fn = || async move {
        //         let values = RemoteV1Client::new(url, None).await;

        //         let agent_dummy_info = AgentDummyInfo {
        //             id: uuid::Uuid::new_v4(),
        //         };
        //         values
        //             .put(
        //                 format!("{PREFIX}{}", &agent_dummy_info.id),
        //                 serde_json::to_string(&agent_dummy_info).unwrap(),
        //             )
        //             .await
        //             .unwrap();

        //         // TODO: wait with a timeout until num_agents have been registered
        //         let handle = {
        //             let mut values = values.clone();
        //             tokio::spawn(async move {
        //                 tokio::select! {
        //                     _ = async {
        //                         let all_agents: Data<AgentDummyInfo> = values.get_pattern(PREFIX, |(_, results)| results.len() >= num_agents_required)
        //                             .await
        //                             .unwrap()
        //                             .into_iter()
        //                             .map(|(key, value)| Ok((key, serde_json::from_str(&value)?)))
        //                             .collect::<Fallible<_>>()
        //                             .unwrap();
        //                         assert!(required_agents <= all_agents.len());
        //                         assert!(all_agents.len() <= num_agents_required);
        //                         eprintln!("{} agents {all_agents:#?}", all_agents.len());
        //                     } => { }
        //                     _ = tokio::time::sleep(Duration::from_millis(50)) => { panic!("not enough agents"); }
        //                 }
        //             })
        //         };

        //         if let Err(e) = handle.await {
        //             panic!("{:#?}", e);
        //         };

        //         // this concludes a checkpoint, possibly providing data, that holochain specific logic can rely on
        //     };
        // }
    }
}
