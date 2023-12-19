#![allow(missing_docs)] // TODO: remove this

//! This module implements value sharing for out-of-band communication between test agents.

use std::collections::BTreeMap;
pub type Data<T> = BTreeMap<String, T>;

pub(crate) mod local_v1 {
    use anyhow::Result as Fallible;
    use std::collections::BTreeMap;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use super::Data;

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

        pub async fn num_waiters(&self) -> usize {
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

    #[cfg(test)]
    mod tests {
        use std::time::Duration;

        use serde::{Deserialize, Serialize};
        use uuid::Uuid;

        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn shared_values_localv1_concurrent() {
            let mut values = LocalV1::default();

            let prefix = "something".to_string();
            let s = "we expect this back".to_string();

            let handle = {
                let prefix = prefix.clone();
                let s = s.clone();
                let mut values = values.clone();

                tokio::spawn({
                    async move {
                        let got: String = values
                            .get_pattern(&prefix, |(_, results)| results.len() > 0)
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
                        let num = values.num_waiters().await;
                        match num {
                            0 => tokio::time::sleep(Duration::from_millis(10)).await,
                            1 => { eprintln!("saw a getter!"); break },
                            _ => panic!("saw more than one waiter"),
                        };
                    }
                } => {
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    panic!("didn't see a waiter");
                }
            };

            values.put(prefix, s).await.unwrap();

            if let Err(e) = handle.await {
                panic!("{:#?}", e);
            };
        }

        #[derive(Debug, Serialize, Deserialize, Clone)]
        struct AgentDummyInfo {
            id: Uuid,
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn shared_values_localv1_simulate_agent_discovery() {
            let values = LocalV1::default();

            const PREFIX: &str = "agent_";

            let required_agents = 2;
            let num_agents = 2;

            let get_handle = {
                let mut values = values.clone();
                tokio::spawn(async move {
                    tokio::select! {
                        _ = async {
                            let all_agents: Data<AgentDummyInfo> = values.get_pattern(PREFIX, |(_, results)| results.len() >= num_agents)
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
                        .put(
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
    use anyhow::Result as Fallible;
    use futures::StreamExt;
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use tokio::task::JoinHandle;

    use holochain_websocket::{WebsocketConfig, WebsocketListener};

    // TODO: this is only used to import the proc macro `SerializedBytes`. figure out how to import that selectively
    use crate::prelude::*;

    use super::local_v1::LocalV1;
    use super::Data;

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
        Put { key: String, value: String },
        Get { pattern: String },
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
                            RequestMessage::Get { pattern } => ResponseMessage::Get(
                                localv1.get_pattern(&pattern, |_| true).await.map_err(|e| {
                                    tracing::error!("{}", e);
                                    e.to_string()
                                }),
                            ),
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
        async fn shared_values_remote_v1_client_basic() {
            let server = RemoteV1Server::new(None).await.unwrap();

            let client = RemoteV1Client::new(server.url(), None).await.unwrap();

            let s = "expecting this back".to_string();

            let r = match client.request(RequestMessage::Test(s.clone())).await {
                Ok(ResponseMessage::Test(r)) => r,
                other => panic!("{other:#?}"),
            };

            assert_eq!(s, r);
        }
    }
}
