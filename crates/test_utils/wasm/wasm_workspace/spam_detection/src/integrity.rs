use hdk::prelude::*;
use holochain_zome_types::warrant::{Warrant, SignedWarrant, WarrantProof};

#[hdk_entry(id = "warrant")]
pub struct WarrantEntry {
    pub signed_warrant: SignedWarrant,
}

#[hdk_extern]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    // Example spam detection: count number of ops from agent in short time
    // (Replace with real logic)
    let is_spam = false; // TODO: implement spam detection
    if is_spam {
        let warrant = Warrant::new_now(
            WarrantProof::SpamDetected,
            op.header().author().clone(),
            op.header().author().clone(),
        );
        let signed_warrant = SignedWarrant::sign(warrant, op.header().author().clone())?;
        create_entry(&WarrantEntry { signed_warrant })?;
        return Ok(ValidateCallbackResult::Invalid("Spam detected, warrant issued".into()));
    }
    // Block agent if warrant exists
    let warrants: Vec<WarrantEntry> = query_warrants_for_agent(op.header().author().clone())?;
    if !warrants.is_empty() {
        return Ok(ValidateCallbackResult::Invalid("Agent blocked due to warrant".into()));
    }
    Ok(ValidateCallbackResult::Valid)
}

fn query_warrants_for_agent(agent: AgentPubKey) -> ExternResult<Vec<WarrantEntry>> {
    let filter = ChainQueryFilter::default().entry_type(EntryType::App(AppEntryType::new("warrant".into(), 0, EntryVisibility::Public)));
    let warrants: Vec<WarrantEntry> = query(filter)?;
    Ok(warrants.into_iter().filter(|w| w.signed_warrant.warrantee == agent).collect())
}

