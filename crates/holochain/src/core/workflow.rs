//! Workflows are the core building block of Holochain functionality.
//!
//! ## Properties
//!
//! Workflows are **transactional**, so that if any workflow fails to run to
//! completion, nothing will happen.
//!
//! In order to achieve this, workflow functions are **free of any side-effects
//! which modify cryptographic state**: they will not modify the source chain
//! nor send network messages which could cause other agents to update their own
//! source chain.
//!
//! Workflows are **never nested**. A workflow cannot call another workflow.
//! However, a workflow can specify that any number of other workflows should
//! be triggered after this one completes.
//!
//! Side effects and triggering of other workflows is specified declaratively
//! rather than imperatively. Each workflow returns a `WorkflowEffects` value
//! representing the side effects that should be run. The `finish` function
//! processes this value and performs the necessary actions, including
//! committing changes to the associated Workspace and triggering other
//! workflows.

mod effects;
pub mod error;
mod genesis_workflow;
mod initialize_zomes_workflow;
mod invoke_zome_workflow;
#[allow(unused_imports)]
pub(crate) use genesis_workflow::*;
pub(crate) use initialize_zomes_workflow::*;
pub(crate) use invoke_zome_workflow::unsafe_invoke_zome_workspace;
pub(crate) use invoke_zome_workflow::*;

pub use effects::*;

use crate::core::state::workspace::Workspace;
use error::*;
use holochain_state::env::EnvironmentWrite;
use must_future::MustBoxFuture;
use tracing::*;

/// Definition of a Workflow.
///
/// The workflow logic is defined in the `workflow` function. Additional
/// parameters can be specified as struct fields on the impls.
///
/// There are three associated types:
/// - Output, the return value of the function
/// - Workspace, the bundle of Buffered Stores used to stage changes to be persisted later
/// - Triggers, a type representing workflows to be triggered upon completion
pub trait Workflow<'env>: Sized + Send {
    /// The return value of the workflow function
    type Output: Send;
    /// The Workspace associated with this Workflow
    type Workspace: Workspace<'env> + 'env;
    /// Represents Workflows to be triggered upon completion
    type Triggers: WorkflowTriggers<'env>;

    /// Defines the actual logic for this Workflow
    fn workflow(
        self,
        workspace: Self::Workspace,
    ) -> MustBoxFuture<'env, WorkflowResult<'env, Self>>;
}

/// This is the main way to run a Workflow. By constructing a Workflow and
/// Workspace, this runs the Workflow and executes the `finish` function on
/// the WorkflowEffects, returning the Output value of the workflow
pub async fn run_workflow<'env, O: Send, Wf: Workflow<'env, Output = O> + 'env>(
    arc: EnvironmentWrite,
    wc: Wf,
    workspace: Wf::Workspace,
) -> WorkflowRunResult<O> {
    let (output, effects) = wc.workflow(workspace).await?;
    finish::<Wf>(arc, effects).await?;
    Ok(output)
}

/// Apply the WorkflowEffects to finalize the Workflow.
/// 1. Persist DB changes via `Workspace::commit_txn`
/// 2. Call any Wasm callbacks
/// 3. Emit any Signals
/// 4. Trigger any subsequent Workflows
async fn finish<'env, Wf: Workflow<'env>>(
    arc: EnvironmentWrite,
    effects: WorkflowEffects<Wf::Workspace, Wf::Triggers>,
) -> WorkflowRunResult<()> {
    let WorkflowEffects {
        workspace,
        triggers,
        callbacks,
        signals,
        ..
    } = effects;

    // finish workspace
    {
        let env = arc.guard().await;
        let writer = env.writer_unmanaged()?;
        workspace.commit_txn(writer)?;
    }

    // finish callbacks
    {
        warn!("Workflow-generated callbacks are unimplemented");
        for _callback in callbacks {
            // TODO
        }
    }

    // finish signals
    {
        warn!("Workflow-generated signals are unimplemented");
        for _signal in signals {
            // TODO
        }
    }

    // finish triggers
    warn!("Workflow-generated triggers are unimplemented");
    let _handle = triggers.run(arc);

    Ok(())
}

impl<'env, Ws: Workspace<'env>, Tr: WorkflowTriggers<'env>> std::fmt::Debug
    for WorkflowEffects<Ws, Tr>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkflowEffects")
            // TODO: Debug repr for triggers
            // .field("triggers", &self.triggers)
            .field("callbacks", &self.callbacks)
            .field("signals", &self.signals)
            .finish()
    }
}
