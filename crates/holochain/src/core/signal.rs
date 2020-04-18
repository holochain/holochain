use holochain_serialized_bytes::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes)]
pub enum Signal {
    Trace,
    // Consistency(ConsistencySignal<String>),
    User(UserSignal),
}

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes)]
pub struct UserSignal;
