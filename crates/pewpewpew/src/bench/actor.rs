pub struct Actor;

impl actix::Actor for Actor {
    type Context = actix::SyncContext<Self>;
}

#[derive(actix::Message)]
#[rtype(result = "()")]
pub(crate) struct Commit(String);

impl From<Commit> for super::command::Commit {
    fn from(c: Commit) -> Self {
        Self::from(c.0)
    }
}

impl From<String> for Commit {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for Commit {
    fn from(s: &str) -> Self {
        Self::from(s.to_string())
    }
}

impl actix::Handler<Commit> for Actor {
    type Result = ();
    fn handle(&mut self, msg: Commit, _ctx: &mut Self::Context) -> Self::Result {
        super::command::commit(msg.into())
    }
}
