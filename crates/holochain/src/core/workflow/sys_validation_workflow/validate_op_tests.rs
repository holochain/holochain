use crate::prelude::DhtOp;
use crate::prelude::Action;
use crate::prelude::DnaDefHashed;
use crate::prelude::DnaDef;
use fixt::prelude::*;
use holochain_state::prelude::CreateFixturator;
use holochain_state::prelude::SignatureFixturator;
use holochain_cascade::MockCascade;
use crate::core::workflow::sys_validation_workflow::validate_op;
use crate::core::MockDhtOpSender;

#[tokio::test(flavor = "multi_thread")]
async fn validate_valid_op() {
    let op = test_op();

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_def = DnaDefHashed::from_content_sync(dna_def);

    let cascade = MockCascade::new();

    validate_op(&op, &dna_def, &cascade, None::<&MockDhtOpSender>).await.unwrap();
}

fn test_op() -> DhtOp {
    let create_action = fixt!(Create);
    let action = Action::Create(create_action);

    DhtOp::RegisterAgentActivity(fixt!(Signature), action)
}
