use holochain_state::prelude::*;

pub struct ActionInfo {
    validation_status: Option<ValidationStatus>,
    when_integrated: Option<Timestamp>,
}
