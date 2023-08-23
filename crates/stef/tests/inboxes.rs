use std::{
    collections::HashMap,
    sync::atomic::{AtomicU32, Ordering},
};

use stef::*;

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    derive_more::From,
    derive_more::Deref,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Id(u32);

impl Id {
    const NEXT_ID: AtomicU32 = AtomicU32::new(0);

    pub fn new() -> Self {
        Self::NEXT_ID.fetch_add(1, Ordering::Relaxed).into()
    }
}

mod convo {
    use std::fmt::Debug;

    use super::*;

    use proptest_derive::Arbitrary;
    use stef::diagram::StateDiagrammable;
    use tokio::sync::mpsc;

    #[derive(Clone, Default, Debug, PartialEq, Eq, Hash)]
    pub struct Convo {
        pub stage: Stage,
        last_important_received: Option<Msg>,
    }

    // impl<S: Debug> Display for Convo<S> {
    //     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    //         f.write_fmt(format_args!(
    //             "{:?}({:?})",
    //             self.stage, self.last_important_received
    //         ))
    //     }
    // }

    #[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
    pub struct DiagramNode(Stage);

    impl From<Convo> for DiagramNode {
        fn from(value: Convo) -> Self {
            Self(value.stage)
        }
    }

    impl From<Msg<String>> for Msg<()> {
        fn from(value: Msg<String>) -> Self {
            match value {
                Msg::Hi => Msg::Hi,
                Msg::HowAreYou => Msg::HowAreYou,
                Msg::ImFine => Msg::ImFine,
                Msg::ThatsGreat => Msg::ThatsGreat,
                Msg::YouShouldSmileMore => Msg::YouShouldSmileMore,
                Msg::HowRude => Msg::HowRude,
                Msg::GottaGo => Msg::GottaGo,
                Msg::KayBye => Msg::KayBye,
                Msg::Sorry(_) => Msg::Sorry(()),
            }
        }
    }

    #[derive(Clone, Copy, Debug, derive_more::Display, PartialEq, Eq, Hash)]
    pub enum Stage {
        SayHello,
        AskStatus,
        Denoument,
        Finished,
    }

    impl Default for Stage {
        fn default() -> Self {
            Stage::SayHello
        }
    }

    impl State for Convo {
        type Action = Action;
        type Effect = Effect;

        fn transition(&mut self, action: Self::Action) -> Self::Effect {
            use Msg::*;
            use Stage::*;

            match action {
                Action::Initiate => {
                    if self.stage == SayHello {
                        self.stage = AskStatus;
                        Effect::reply(vec![Hi])
                    } else {
                        Effect::reply(vec![])
                    }
                }
                Action::Msg(msg) => {
                    let effect = match (&self.stage, &self.last_important_received, &msg) {
                        (Finished, _, _) => return Effect::reply(vec![KayBye]),

                        (_, recv, msg) if recv.as_ref() == Some(msg) => {
                            self.stage = Finished;
                            return Effect::close(
                                vec![Sorry("you repeated your last message".into())],
                                CloseReason::ProtocolBreach,
                            );
                        }

                        (SayHello, _, Hi) => {
                            self.stage = AskStatus;
                            Effect::reply(vec![Hi])
                        }

                        (AskStatus, _, Hi) => {
                            self.stage = Denoument;
                            Effect::reply(vec![HowAreYou])
                        }

                        (AskStatus, _, HowAreYou) => {
                            self.stage = Denoument;
                            Effect::reply(vec![ImFine, HowAreYou])
                        }

                        (AskStatus | Denoument, _, ImFine) => Effect::reply(vec![ThatsGreat]),
                        (AskStatus | Denoument, _, ThatsGreat) => Effect::reply(vec![]),

                        (Denoument, _, HowAreYou) => Effect::reply(vec![ImFine, GottaGo]),

                        (_, _, GottaGo) => {
                            self.stage = Finished;
                            Effect::close(vec![KayBye], CloseReason::End)
                        }
                        (_, _, KayBye | Sorry(_)) => {
                            self.stage = Finished;
                            Effect::close(vec![], CloseReason::End)
                        }

                        (_, _, YouShouldSmileMore) => {
                            self.stage = Finished;
                            Effect::close(vec![HowRude, KayBye], CloseReason::Block)
                        }

                        (_, _, msg) => {
                            self.stage = Finished;
                            Effect::close(
                                vec![Sorry(format!("unexpected message: {msg:?}"))],
                                CloseReason::ProtocolBreach,
                            )
                        }
                    };
                    if !matches!(msg, ThatsGreat | ImFine) {
                        self.last_important_received = Some(msg);
                    }
                    effect
                }
            }
        }
    }

    impl StateDiagrammable for Convo {
        type Node = DiagramNode;
        type Edge = Action<()>;
    }

    #[derive(
        Clone,
        Debug,
        PartialEq,
        Eq,
        Hash,
        serde::Serialize,
        serde::Deserialize,
        derive_more::From,
        Arbitrary,
    )]
    pub enum Action<S: Eq + Send + Sync = String> {
        Msg(Msg<S>),
        Initiate,
    }

    impl From<Action<String>> for Action<()> {
        fn from(value: Action<String>) -> Self {
            match value {
                Action::Msg(msg) => Self::Msg(msg.into()),
                Action::Initiate => Self::Initiate,
            }
        }
    }

    #[derive(
        Clone,
        Debug,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        serde::Serialize,
        serde::Deserialize,
        Arbitrary,
    )]
    pub enum Msg<S: Eq + Send + Sync = String> {
        Hi,
        HowAreYou,
        ImFine,
        ThatsGreat,
        YouShouldSmileMore,
        HowRude,
        GottaGo,
        KayBye,
        Sorry(S),
    }

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum MsgEdge {
        Hi,
        HowAreYou,
        ImFine,
        ThatsGreat,
        YouShouldSmileMore,
        HowRude,
        GottaGo,
        KayBye,
        Sorry,
    }

    impl From<Msg> for MsgEdge {
        fn from(value: Msg) -> Self {
            match value {
                Msg::Sorry(_) => Self::Sorry,
                Msg::Hi => Self::Hi,
                Msg::HowAreYou => Self::HowAreYou,
                Msg::ImFine => Self::ImFine,
                Msg::ThatsGreat => Self::ThatsGreat,
                Msg::YouShouldSmileMore => Self::YouShouldSmileMore,
                Msg::HowRude => Self::HowRude,
                Msg::GottaGo => Self::GottaGo,
                Msg::KayBye => Self::KayBye,
            }
        }
    }

    impl ActionCompact for Action {}

    #[derive(Debug, PartialEq, Eq)]
    pub struct Effect {
        pub msgs: Vec<Msg>,
        pub close: Option<CloseReason>,
    }

    impl Effect {
        pub fn reply(msgs: Vec<Msg>) -> Self {
            Self { msgs, close: None }
        }

        pub fn close(msgs: Vec<Msg>, reason: CloseReason) -> Self {
            Self {
                msgs,
                close: Some(reason),
            }
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    pub enum CloseReason {
        End,
        Block,
        ProtocolBreach,
    }

    #[derive(Clone, derive_more::Deref, derive_more::DerefMut)]
    pub struct Sender(
        Id,
        #[deref]
        #[deref_mut]
        mpsc::Sender<Action>,
    );
    pub type Receiver = mpsc::Receiver<Action>;

    impl PartialEq for Sender {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }

    impl Eq for Sender {}

    impl Sender {
        pub fn new(tx: mpsc::Sender<Action>) -> Self {
            Self(Id::new(), tx)
        }
    }
}

mod peerlist {
    use super::*;

    #[derive(Default)]
    pub struct PeerList {
        peers: HashMap<Id, Peer>,
    }

    impl State for PeerList {
        type Action = Action;
        type Effect = Result<Effect, Error>;

        fn transition(&mut self, a: Self::Action) -> Self::Effect {
            match a {
                Action::StartConvo(id) => {
                    let peer = self.peers.get(&id).ok_or_else(|| Error::NoSuchPeer(id))?;
                    if peer.convo.is_some() {
                        Err(Error::ConvoAlreadyStarted(id))
                    } else {
                        Ok(Effect::SendMsg(peer.sender.clone(), convo::Msg::Hi))
                    }
                }
                Action::EndConvo(_) => todo!(),
            }
        }
    }

    struct Peer {
        sender: convo::Sender,
        convo: Option<convo::Convo>,
    }

    #[derive(Clone, serde::Serialize, serde::Deserialize)]
    pub enum Action {
        StartConvo(Id),
        EndConvo(Id),
    }

    #[derive(PartialEq, Eq)]
    pub enum Effect {
        SendMsg(convo::Sender, convo::Msg),
    }

    #[derive(Debug, PartialEq, Eq)]
    pub enum Error {
        NoSuchPeer(Id),
        ConvoAlreadyStarted(Id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use convo::Msg::*;
    use convo::{Effect, *};
    use stef::diagram::StateDiagrammable;
    use tokio::task::JoinHandle;

    #[test]
    #[ignore = "run manually"]
    fn convo_diagram() {
        use petgraph::dot::Dot;
        let graph = Convo::default().state_diagram(256, 8);
        let dot = format!("{:?}", Dot::with_config(&graph, &[]));
        std::fs::write("./convo.dot", dot).unwrap();
    }

    #[test]
    fn convo_happy_path_solo() {
        {
            let mut convo = StoreEffects::new(Convo::default(), 10);
            convo.transition(Hi.into());
            convo.transition(HowAreYou.into());
            convo.transition(GottaGo.into());

            assert_eq!(
                convo.drain_effects(),
                vec![
                    Effect::reply(vec![Hi]),
                    Effect::reply(vec![ImFine, HowAreYou]),
                    Effect::close(vec![KayBye], CloseReason::End)
                ]
            )
        }
        {
            let mut convo = StoreEffects::new(Convo::default(), 10);
            convo.transition(Hi.into());
            convo.transition(HowAreYou.into());
            convo.transition(ImFine.into());
            convo.transition(YouShouldSmileMore.into());

            assert_eq!(
                convo.drain_effects(),
                vec![
                    Effect::reply(vec![Hi]),
                    Effect::reply(vec![ImFine, HowAreYou]),
                    Effect::reply(vec![ThatsGreat]),
                    Effect::close(vec![HowRude, KayBye], CloseReason::Block)
                ]
            )
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn convo_duo() {
        let ((ta, ja), (_, jb)) = duo();
        ta.send(convo::Action::Initiate).await.unwrap();

        let (ra, rb) = futures::join!(ja, jb);
        assert_eq!(ra.unwrap().unwrap(), Ok(convo::CloseReason::End));
        assert_eq!(rb.unwrap().unwrap(), Ok(convo::CloseReason::End));
    }

    proptest::proptest! {
        #[test]
        #[allow(unreachable_code)]
        fn convo_fuzz(msgs: Vec<convo::Msg>) {
            let mut honest = convo::Convo::default();

            if msgs.len() < 8 {
                return Ok(())
            }
            println!("msgs to get through: {}", msgs.len());
            for msg in msgs {
                let (eff, disp) = transition_info(&mut honest, msg.into(), "a", false);
                println!("{disp}");
                if eff.close.is_some() {
                    println!("---------CLOSE---------");
                    return Ok(());
                }
            }
            panic!("didn't terminate convo");
        }
    }

    fn task(
        tx: convo::Sender,
        mut rx: convo::Receiver,
        mut convo: Convo,
        id: &str,
        right: bool,
    ) -> JoinHandle<Result<CloseReason, ()>> {
        let id = id.to_string();
        tokio::spawn(async move {
            while let Some(action) = rx.recv().await {
                let (eff, disp) = transition_info(&mut convo, action, &id, right);
                println!("{disp}");

                for msg in eff.msgs {
                    tx.send(msg.into()).await.unwrap();
                }

                if let Some(reason) = eff.close {
                    return Ok(reason);
                }
            }
            Err(())
        })
    }

    fn transition_info(
        convo: &mut Convo,
        action: convo::Action,
        id: &str,
        right: bool,
    ) -> (convo::Effect, String) {
        let stage_before = format!("{:?}", convo.stage);
        let eff = convo.transition(action.clone().into());
        let stage_after = format!("{:?}", convo.stage);

        let msg_display = match action {
            convo::Action::Msg(msg) => {
                let disp = format!("{msg:?}");
                if right {
                    format!("{disp:>20}")
                } else {
                    format!("{disp:<20}")
                }
            }
            _ => {
                let disp = format!("{action:?}");
                format!("{disp:*^20}")
            }
        };
        let disp = if stage_before != stage_after {
            format!(
                "{msg_display:20} --> [{id}] ({stage_before:10} -> {stage_after:10}) :=> {eff:?}"
            )
        } else {
            format!("{msg_display:20} --> [{id}]                            :=> {eff:?}")
        };
        (eff, disp)
    }

    fn duo() -> (
        (
            convo::Sender,
            tokio::time::Timeout<JoinHandle<Result<CloseReason, ()>>>,
        ),
        (
            convo::Sender,
            tokio::time::Timeout<JoinHandle<Result<CloseReason, ()>>>,
        ),
    ) {
        let (ta, ra) = tokio::sync::mpsc::channel(1);
        let (tb, rb) = tokio::sync::mpsc::channel(1);
        let ta = Sender::new(ta);
        let tb = Sender::new(tb);

        let timeout = tokio::time::Duration::from_secs(1);
        let task_a =
            tokio::time::timeout(timeout, task(tb.clone(), ra, Convo::default(), "A", true));
        let task_b =
            tokio::time::timeout(timeout, task(ta.clone(), rb, Convo::default(), "B", false));

        ((ta, task_a), (tb, task_b))
    }
}

/*

            tokio::runtime::Builder::new_multi_thread().build().unwrap().spawn(async move {

                let ((ta, ja), (_, jb)) = duo();
                dbg!(&msg);
            });
*/
