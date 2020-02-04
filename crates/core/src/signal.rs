
pub enum Signal {
    Trace,
    // Consistency(ConsistencySignal<String>),
    User(UserSignal),
}

pub struct UserSignal;
