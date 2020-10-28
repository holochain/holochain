use super::*;

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
