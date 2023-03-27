use std::sync::Arc;

use lens_rs::*;

use crate::prelude::*;
use ::fixt::fixt;

use super::*;

#[test]
fn lenses() {
    let mp = Arc::new(().try_into().unwrap());

    let a = Action::AgentValidationPkg(AgentValidationPkg {
        author: fixt!(AgentPubKey),
        timestamp: fixt!(Timestamp),
        action_seq: 1,
        prev_action: fixt!(ActionHash),
        membrane_proof: Some(mp),
    });

    assert_eq!(a.view(optics!(AgentValidationPkg.action_seq)), 1);
    assert_eq!(
        a.preview(optics!(AgentValidationPkg.membrane_proof)),
        Some(mp)
    );
}
