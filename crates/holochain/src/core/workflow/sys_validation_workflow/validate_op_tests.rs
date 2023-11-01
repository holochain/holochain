use crate::prelude::DhtOp;
use crate::prelude::Action;
use crate::prelude::DnaDefHashed;
use crate::prelude::DnaDef;
use fixt::prelude::*;
use holochain_state::prelude::CreateFixturator;
use holochain_state::prelude::SignatureFixturator;
use holochain_cascade::MockCascade;

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_op() {
    let op = test_op();

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let cascade = MockCascade::new();

    
}

fn test_op() -> DhtOp {
    let mut create_action = fixt!(Create);
    let action = Action::Create(create_action);

    DhtOp::RegisterAgentActivity(fixt!(Signature), action)
}
