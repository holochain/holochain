use super::*;
use futures::sink::SinkExt;

/// a small debug representation of another type
#[derive(Clone, PartialEq)]
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
    Arc<KitsuneOpHash> => |s| {
        let f = format!("{:?}", s);
        format!("o{}", &f[13..25])
    },
}

/// an event type for an event emitted by the test suite harness
#[derive(Clone, Debug)]
pub enum HarnessEventType {
    Close,
    Join {
        space: Slug,
        agent: Slug,
    },
    StoreAgentInfo {
        agent: Slug,
        agent_info: Arc<AgentInfoSigned>,
    },
    Call {
        space: Slug,
        to_agent: Slug,
        payload: String,
    },
    Notify {
        space: Slug,
        to_agent: Slug,
        payload: String,
    },
    Gossip {
        op_hash: Slug,
        op_data: String,
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
            while let Ok(evt) = trace_recv.recv().await {
                let HarnessEvent { nick, ty } = evt;
                const T: &str = "HARNESS_EVENT";
                tracing::debug!(
                    %T,
                    %nick,
                    ?ty,
                );
                if let HarnessEventType::Close = ty {
                    return;
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
            new_nick.push('.');
        }
        new_nick.push_str(sub_nick.as_ref());
        Self {
            nick: Arc::new(new_nick),
            chan: self.chan.clone(),
        }
    }

    /// close this channel.
    pub fn close(&self) {
        self.publish(HarnessEventType::Close);
    }

    /// break off a broadcast receiver. this receiver will not get historical
    /// messages... only those that are emitted going forward
    pub fn receive(&self) -> impl tokio_stream::Stream<Item = HarnessEvent> {
        let (mut s, r) = futures::channel::mpsc::channel(10);
        let mut chan = self.chan.subscribe();
        tokio::task::spawn(async move {
            while let Ok(msg) = chan.recv().await {
                let is_close = matches!(&msg.ty, HarnessEventType::Close);
                if s.send(msg).await.is_err() {
                    break;
                }
                if is_close {
                    break;
                }
            }
            s.close_channel();
        });
        r
    }

    /// publish a harness event to all receivers
    pub fn publish(&self, ty: HarnessEventType) {
        let _ = self.chan.send(HarnessEvent {
            nick: self.nick.clone(),
            ty,
        });
    }
}
