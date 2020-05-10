use crate::core::ribosome::AllowSideEffects;
use crate::core::ribosome::FnComponents;
use crate::core::ribosome::Invocation;
use holochain_serialized_bytes::prelude::*;
use holochain_types::dna::Dna;
use holochain_zome_types::migrate_agent::MigrateAgent;
use holochain_zome_types::migrate_agent::MigrateAgentCallbackResult;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;

#[derive(Clone)]
pub struct MigrateAgentInvocation {
    dna: Dna,
    migrate_agent: MigrateAgent,
}

impl Invocation for MigrateAgentInvocation {
    fn allow_side_effects(&self) -> AllowSideEffects {
        AllowSideEffects::No
    }
    fn zome_names(&self) -> Vec<ZomeName> {
        self.dna.zomes.keys().cloned().collect()
    }
    fn fn_components(&self) -> FnComponents {
        vec![
            "migrate_agent".into(),
            match self.migrate_agent {
                MigrateAgent::Open => "open",
                MigrateAgent::Close => "close",
            }
            .into(),
        ]
        .into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(HostInput::new((&self.migrate_agent).try_into()?))
    }
}

impl TryFrom<MigrateAgentInvocation> for HostInput {
    type Error = SerializedBytesError;
    fn try_from(migrate_agent_invocation: MigrateAgentInvocation) -> Result<Self, Self::Error> {
        Ok(Self::new(
            (&migrate_agent_invocation.migrate_agent).try_into()?,
        ))
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

impl From<Vec<MigrateAgentCallbackResult>> for MigrateAgentResult {
    fn from(callback_results: Vec<MigrateAgentCallbackResult>) -> Self {
        callback_results.into_iter().fold(Self::Pass, |acc, x| {
            match x {
                // fail always overrides the acc
                MigrateAgentCallbackResult::Fail(zome_name, fail_string) => {
                    Self::Fail(zome_name, fail_string)
                }
                // pass allows the acc to continue
                MigrateAgentCallbackResult::Pass => acc,
            }
        })
    }
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
//     //     payload: HostInput::new(self.dna().try_into()?),
//     // };
//     // let callback_outputs: Vec<Option<GuestOutput>> =
//     //     self.run_callback(callback_invocation, false)?;
//     // assert_eq!(callback_outputs.len(), 2);
//
//     for callback_output in self.call_iterator(migrate_agent_invocation.into()) {
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
