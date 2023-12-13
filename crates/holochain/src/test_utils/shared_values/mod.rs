#![allow(missing_docs)] // TODO: remove this

//! This module implements value sharing for out-of-band communication between test agents.

use anyhow::{bail, Result as Fallible};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{collections::HashMap, sync::Arc};

const TEST_SHARED_VALUES_TYPE: &str = "TEST_SHARED_VALUES_TYPE";
const TEST_SHARED_VALUES_TYPE_LOCALV1: &str = "localv1";
const TEST_SHARED_VALUES_TYPE_REMOTEV1: &str = "remotev1";
const TEST_SHARED_VALUES_REMOTEV1_URL: &str = "TEST_SHARED_VALUES_REMOTEV1_URL";

/// Local implementation using a guarded HashMap as its datastore.
#[derive(Clone, Default)]
pub struct LocalV1 {
    num_waiters: Arc<AtomicUsize>,
    data: Arc<tokio::sync::Mutex<HashMap<String, String>>>,
    notification: Arc<tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Notify>>>>,
}

/// Remote implementation using Websockets for data passing.
#[derive(Clone)]
pub struct RemoteV1Client {
    url: url2::Url2,
    sender: Arc<holochain_websocket::WebsocketSender>,
    receiver: Arc<holochain_websocket::WebsocketReceiver>,
}

/// Remote implementation using Websockets for data passing.
#[derive(Clone)]
pub struct RemoteV1Server {
    url: url2::Url2,
    sender: Arc<holochain_websocket::WebsocketSender>,
    receiver: Arc<holochain_websocket::WebsocketReceiver>,
}

#[derive(Clone)]
pub enum SharedValues {
    LocalV1(LocalV1),
    RemoteV1Client(RemoteV1Client),
}

impl SharedValues {
    /// Returns a new MessageBus by respecting the environment variables:
    /// TEST_SHARED_VALUES_TYPE: can be either of
    /// - `localv1`: creates a message bus for in-process messaging
    /// - `remotev1`: creates a message bus for inter-process messaging. relies on another environment variable:
    ///     - TEST_SHARED_VALUES_REMOTEV1_URL: a URL for the remote endpoint to connect the message bus to
    pub async fn new_from_env() -> Fallible<Self> {
        let bus_type = std::env::var(TEST_SHARED_VALUES_TYPE)
            .unwrap_or(TEST_SHARED_VALUES_TYPE_LOCALV1.to_string());

        match bus_type.as_str() {
            TEST_SHARED_VALUES_TYPE_LOCALV1 => Ok(Self::LocalV1(LocalV1::default())),
            TEST_SHARED_VALUES_TYPE_REMOTEV1 => {
                let url_string = std::env::var(TEST_SHARED_VALUES_REMOTEV1_URL)?;
                let url = url2::Url2::try_parse(url_string)?;

                let (sender, receiver) =
                    holochain_websocket::connect(url.clone(), Default::default()).await?;

                Ok(Self::RemoteV1Client(RemoteV1Client {
                    url,
                    sender: Arc::new(sender),
                    receiver: Arc::new(receiver),
                }))
            }

            bus_type => {
                bail!("unknown message bus type: {bus_type}")
            }
        }
    }

    pub async fn num_waiters(&self) -> usize {
        match self {
            SharedValues::LocalV1(LocalV1 { num_waiters, .. }) => {
                num_waiters.load(Ordering::SeqCst)
            }

            _ => unimplemented!(),
        }
    }

    /// Gets the `value` for `key`; waits for it to become available if necessary.
    pub async fn get<T: for<'a> Deserialize<'a>>(
        &mut self,
        key: &str,
        mut ignore_existing: bool,
    ) -> Fallible<T> {
        match self {
            SharedValues::LocalV1(localv1) => {
                loop {
                    let notifier =
                                // new scope so data_guard gets dropped before waiting for a notification
                                {
                                    let data_guard = localv1.data.lock().await;

                                    if !ignore_existing {
                                        if let Some(value) = data_guard.get(key) {
                                            return Ok(serde_json::from_str(value)?);
                                        }
                                    }

                                    // get the notifier while still holding the data lock.
                                    // this prevents a race between getting the notifier and a writer just writing something and sending notifications for it
                                    localv1.num_waiters.fetch_add(1, Ordering::SeqCst);
                                    localv1
                                        .notification
                                        .lock()
                                        .await
                                        .entry(key.to_string())
                                        .or_default()
                                        .clone()
                                };

                    notifier.notified().await;
                    ignore_existing = false;

                    localv1.num_waiters.fetch_sub(1, Ordering::SeqCst);
                }
            }
            SharedValues::RemoteV1Client(_) => unimplemented!(),
        }
    }

    /// Gets all values that have a matching key prefix; waits for `min_results` to become available if specified.
    pub async fn get_pattern<T: for<'a> Deserialize<'a>>(
        &mut self,
        pattern: &str,
        mut ignore_existing: bool,
        maybe_min_results: Option<usize>,
    ) -> Fallible<HashMap<String, T>> {
        match self {
            SharedValues::LocalV1(localv1) => {
                loop {
                    let notifier =
                                // new scope so data_guard gets dropped before waiting for a notification
                                {
                                    let data_guard = localv1.data.lock().await;


                                    if !ignore_existing {
                                        let mut results: HashMap<String, T>  = Default::default();

                                        for (key, value) in data_guard.iter() {
                                            if key.matches(pattern).count() > 0 {
                                                results.insert(key.to_string(), serde_json::from_str(&value)?);
                                            }
                                        }

                                        if let Some(min_results) = maybe_min_results {
                                            if results.len() >= min_results {
                                                return Ok(results);
                                            }
                                        }
                                    }

                                    // get the notifier while still holding the data lock.
                                    // this prevents a race between getting the notifier and a writer just writing something and sending notifications for it
                                    localv1.num_waiters.fetch_add(1, Ordering::SeqCst);
                                    localv1
                                        .notification
                                        .lock()
                                        .await
                                        .entry(pattern.to_string())
                                        .or_default()
                                        .clone()
                                };

                    notifier.notified().await;
                    ignore_existing = false;

                    localv1.num_waiters.fetch_sub(1, Ordering::SeqCst);
                }
            }
            SharedValues::RemoteV1Client(_) => unimplemented!(),
        }
    }

    /// Puts the `value` for `key` and notifies any waiters if there are any.
    pub async fn put<T: Serialize + for<'a> Deserialize<'a>>(
        &mut self,
        key: String,
        value: T,
    ) -> Fallible<Option<T>> {
        match self {
            SharedValues::LocalV1(localv1) => {
                let mut data_guard = localv1.data.lock().await;

                let maybe_previous = if let Some(previous_serialized) =
                    data_guard.insert(key.clone(), serde_json::to_string(&value)?)
                {
                    Some(serde_json::from_str(&previous_serialized)?)
                } else {
                    None
                };

                for (pattern, notifier) in localv1.notification.lock().await.iter() {
                    if key.matches(pattern).count() > 0 {
                        eprintln!("{key} matched by {pattern}");
                        notifier.notify_waiters();
                    } else {
                        eprintln!("{key} not matched by {pattern}");
                    }
                }

                Ok(maybe_previous)
            }
            SharedValues::RemoteV1Client(_) => unimplemented!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use uuid::Uuid;

    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shared_values_localv1_concurrent() {
        let mut values = SharedValues::LocalV1(LocalV1::default());

        let prefix = "something".to_string();
        let s = "we expect this back".to_string();

        let handle = {
            let prefix = prefix.clone();
            let s = s.clone();
            let mut values = values.clone();

            tokio::spawn({
                async move {
                    let got: String = values.get(&prefix, true).await.unwrap();
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
        let values = SharedValues::LocalV1(LocalV1::default());

        const PREFIX: &str = "agent_";

        let required_agents = 2;
        let num_agents = 2;

        let get_handle = {
            let mut values = values.clone();
            tokio::spawn(async move {
                tokio::select! {
                    _ = async {
                        let all_agents: HashMap<_, AgentDummyInfo> = values.get_pattern(PREFIX, false, Some(num_agents)).await.unwrap();
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
                        agent_dummy_info,
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
