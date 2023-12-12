#![allow(missing_docs)] // TODO: remove this

//! This module implements value sharing for out-of-band communication between test agents.

use anyhow::{bail, Result as Fallible};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

static TEST_SHARED_VALUES_TYPE: &str = "TEST_SHARED_VALUES_TYPE";
static TEST_SHARED_VALUES_TYPE_LOCALV1: &str = "localv1";
static TEST_SHARED_VALUES_TYPE_REMOTEV1: &str = "remotev1";
static TEST_SHARED_VALUES_REMOTEV1_URL: &str = "TEST_SHARED_VALUES_REMOTEV1_URL";

/// Local implementation using a guarded HashMap as its datastore.
#[derive(Clone, Default)]
pub struct LocalV1 {
    num_waiters: Arc<AtomicUsize>,
    data: Arc<tokio::sync::Mutex<HashMap<String, String>>>,
    notification: Arc<tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Notify>>>>,
}

/// Remote implementation using Websockets for data passing.
#[derive(Clone)]
pub struct RemoteV1 {
    url: url2::Url2,
    sender: Arc<holochain_websocket::WebsocketSender>,
    receiver: Arc<holochain_websocket::WebsocketReceiver>,
}

#[derive(Clone)]
pub enum SharedValues {
    LocalV1(LocalV1),
    RemoteV1(RemoteV1),
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

        if &bus_type == TEST_SHARED_VALUES_TYPE_LOCALV1 {
            Ok(Self::LocalV1(LocalV1::default()))
        } else if &bus_type == TEST_SHARED_VALUES_TYPE_REMOTEV1 {
            let url_string = std::env::var(TEST_SHARED_VALUES_REMOTEV1_URL)?;
            let url = url2::Url2::try_parse(url_string)?;

            let (sender, receiver) =
                holochain_websocket::connect(url.clone(), Default::default()).await?;

            Ok(Self::RemoteV1(RemoteV1 {
                url,
                sender: Arc::new(sender),
                receiver: Arc::new(receiver),
            }))
        } else {
            bail!("unknown message bus type: {bus_type}")
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
    pub async fn get<T: for<'a> Deserialize<'a>>(&mut self, key: &str) -> Fallible<T> {
        match self {
            SharedValues::LocalV1(localv1) => {
                loop {
                    let notifier =
                                // new scope so data_guard gets dropped before waiting for a notification
                                {
                                    let data_guard = localv1.data.lock().await;

                                    if let Some(value) = data_guard.get(key) {
                                        return Ok(serde_json::from_str(value)?);
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

                    localv1.num_waiters.fetch_sub(1, Ordering::SeqCst);
                }
            }
            SharedValues::RemoteV1(_) => unimplemented!(),
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

                if let Some(notifier) = localv1.notification.lock().await.get(&key) {
                    notifier.notify_waiters();
                }

                Ok(maybe_previous)
            }
            SharedValues::RemoteV1(_) => unimplemented!(),
        }
    }
}

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
                let got: String = values.get(&prefix).await.unwrap();
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
                    0 => tokio::time::sleep(Duration::from_millis(100)).await,
                    1 => break,
                    _ => panic!("saw more than one waiter"),
                };
            }
        } => {
        }
        _ = tokio::time::sleep(Duration::from_secs(1)) => {
            panic!("didn't see a waiter");
        }
    };

    values.put(prefix, s).await.unwrap();

    if let Err(e) = handle.await {
        panic!("{:#?}", e);
    };
}
