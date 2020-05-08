use holochain_types::nucleus::ZomeName;
use crate::core::ribosome::guest_callback::Invocation;
use crate::core::ribosome::guest_callback::CallbackFnComponents;
use holochain_zome_types::migrate_agent::MigrateAgent;
use core::convert::TryFrom;
use holochain_zome_types::CallbackHostInput;
use holochain_serialized_bytes::prelude::*;

pub struct MigrateAgentInvocation<'a> {
    zome_name: ZomeName,
    migrate_agent: &'a MigrateAgent,
}

impl Invocation for &MigrateAgentInvocation<'_> { }

impl From<&MigrateAgentInvocation<'_>> for ZomeName {
    fn from(migrate_agent_invocation: &MigrateAgentInvocation) -> ZomeName {
        migrate_agent_invocation.zome_name
    }
}

impl From<&MigrateAgentInvocation<'_>> for CallbackFnComponents {
    fn from(migrate_agent_invocation: &MigrateAgentInvocation) -> CallbackFnComponents {
        CallbackFnComponents(vec!["migrate_agent".into(), match migrate_agent_invocation.migrate_agent {
            MigrateAgent::Open(_) => "open",
            MigrateAgent::Close(_) => "close",
        }.into(),
        format!("{}", holo_hash::DnaHash::from(match migrate_agent_invocation.migrate_agent {
            MigrateAgent::Open(dna_hash) => dna_hash,
            MigrateAgent::Close(dna_hash) => dna_hash,
        }.to_owned()))])
    }
}

impl TryFrom<&MigrateAgentInvocation<'_>> for CallbackHostInput {
    type Error = SerializedBytesError;
    fn try_from(migrate_agent_invocation: &MigrateAgentInvocation) -> Result<Self, Self::Error> {
        Ok(CallbackHostInput::new(migrate_agent_invocation.migrate_agent.try_into()?))
    }
}

/// the aggregate result of all zome callbacks for migrating an agent between dnas
pub enum MigrateAgentResult {
    /// all implemented migrate agent callbacks in all zomes passed
    Pass,
    /// some migrate agent callback failed
    /// ZomeName is the first zome that failed
    /// String is some human readable string explaining the failure
    Fail(ZomeName, String),
}


        // let mut agent_migrate_dna_result = MigrateAgentDnaResult::Pass;
        //
        // // we need to ask every zome in order if the agent is ready to migrate
        // 'zomes: for zome_name in self.dna().zomes.keys() {
        //     let migrate_agent_invocation = MigrateAgentInvocation {
        //         zome_name: &zome_name,
        //         // @todo - don't send the whole dna into the wasm?? maybe dna def if/when it lands
        //         dna: self.dna(),
        //     };
        //     // let callback_invocation = CallbackInvocation {
        //     //     components: vec![
        //     //         "migrate_agent".into(),
        //     //         match agent_migrate_direction {
        //     //             MigrateAgentDirection::Open => "open",
        //     //             MigrateAgentDirection::Close => "close",
        //     //         }
        //     //         .into(),
        //     //     ],
        //     //     zome_name: zome_name.to_string(),
        //     //     payload: CallbackHostInput::new(self.dna().try_into()?),
        //     // };
        //     // let callback_outputs: Vec<Option<CallbackGuestOutput>> =
        //     //     self.run_callback(callback_invocation, false)?;
        //     // assert_eq!(callback_outputs.len(), 2);
        //
        //     for callback_output in self.callback_iterator(migrate_agent_invocation.into()) {
        //         agent_migrate_dna_result = match callback_output {
        //             // if a callback is implemented try to deserialize the result
        //             Some(implemented) => {
        //                 match MigrateAgentCallbackResult::try_from(implemented.into_inner()) {
        //                     Ok(v) => match v {
        //                         // if a callback passes keep the current dna result
        //                         MigrateAgentCallbackResult::Pass => agent_migrate_dna_result,
        //                         // if a callback fails then the dna migrate needs to fail
        //                         MigrateAgentCallbackResult::Fail(fail_string) => {
        //                             MigrateAgentDnaResult::Fail(zome_name.to_string(), fail_string)
        //                         }
        //                     },
        //                     // failing to deserialize an implemented callback result is a fail
        //                     Err(e) => MigrateAgentDnaResult::Fail(
        //                         zome_name.to_string(),
        //                         format!("{:?}", e),
        //                     ),
        //                 }
        //             }
        //             // if a callback is not implemented keep the current dna result
        //             None => agent_migrate_dna_result,
        //         };
        //
        //         // if dna result has failed due to _any_ zome we need to break the outer loop for
        //         // all zomes
        //         match agent_migrate_dna_result {
        //             MigrateAgentDnaResult::Fail(_, _) => break 'zomes,
        //             _ => {}
        //         }
        //     }
        // }
        //
        // Ok(agent_migrate_dna_result)
