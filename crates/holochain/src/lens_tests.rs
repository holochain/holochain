use std::sync::Arc;

use lens_rs::*;

use crate::prelude::*;
use ::fixt::fixt;

#[test]
fn lenses() {
    let mp: Arc<SerializedBytes> = Arc::new(().try_into().unwrap());
    let mp2 = mp.clone();
    let author = fixt!(AgentPubKey);

    let a = Action::AgentValidationPkg(AgentValidationPkg {
        author: author.clone(),
        timestamp: fixt!(Timestamp),
        action_seq: 2,
        prev_action: fixt!(ActionHash),
        membrane_proof: Some(mp),
    });

    assert_eq!(
        a.preview_ref(optics!(AgentValidationPkg.action_seq)),
        Some(&2)
    );
    assert_eq!(
        a.preview_ref(optics!(AgentValidationPkg.author)),
        Some(&author)
    );
    assert_eq!(
        a.preview(optics!(AgentValidationPkg.membrane_proof)),
        Some(Some(mp2))
    );
}
