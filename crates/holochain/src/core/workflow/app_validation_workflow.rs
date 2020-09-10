//! The workflow and queue consumer for sys validation

use std::{convert::TryInto, sync::Arc};

use super::{
    error::WorkflowResult,
    integrate_dht_ops_workflow::reintegrate_single_data,
    integrate_dht_ops_workflow::{
        disintegrate_single_data, disintegrate_single_metadata, integrate_single_data,
        integrate_single_metadata,
    },
    produce_dht_ops_workflow::dht_op_light::light_to_op,
};
use crate::{
    conductor::api::CellConductorApiT,
    core::present::retrieve_element,
    core::present::retrieve_entry,
    core::present::DataSource,
    core::present::DbPair,
    core::ribosome::guest_callback::validate_link_add::ValidateLinkAddHostAccess,
    core::ribosome::guest_callback::validate_link_add::ValidateLinkAddInvocation,
    core::ribosome::guest_callback::validate_link_add::ValidateLinkAddResult,
    core::ribosome::wasm_ribosome::WasmRibosome,
    core::state::cascade::Cascade,
    core::{
        queue_consumer::{OneshotWriter, TriggerSender, WorkComplete},
        ribosome::guest_callback::validate::ValidateHostAccess,
        ribosome::guest_callback::validate::ValidateInvocation,
        ribosome::guest_callback::validate::ValidateResult,
        ribosome::RibosomeT,
        state::{
            dht_op_integration::{
                IntegratedDhtOpsStore, IntegrationLimboStore, IntegrationLimboValue,
            },
            element_buf::ElementBuf,
            metadata::MetadataBuf,
            validation_db::{ValidationLimboStatus, ValidationLimboStore, ValidationLimboValue},
            workspace::{Workspace, WorkspaceResult},
        },
        validation::DepType,
        validation::PendingDependencies,
    },
};
use either::Either;
use error::AppValidationResult;
pub use error::*;
use fallible_iterator::FallibleIterator;
use holo_hash::{AnyDhtHash, DhtOpHash};
use holochain_p2p::HolochainP2pCell;
use holochain_state::{
    buffer::{BufferedStore, KvBufFresh},
    db::{INTEGRATED_DHT_OPS, INTEGRATION_LIMBO},
    error::DatabaseResult,
    fresh_reader,
    prelude::*,
};
use holochain_types::{
    dht_op::DhtOp, dht_op::DhtOpLight, dna::DnaFile, test_utils::which_agent,
    validate::ValidationStatus, Entry, HeaderHashed, Timestamp,
};
use holochain_zome_types::{
    element::Element, element::SignedHeaderHashed, header::AppEntryType, header::EntryType,
    header::LinkAdd, zome::ZomeName, Header,
};
use tracing::*;
use types::*;

#[cfg(test)]
mod tests;

mod error;
mod types;

#[instrument(skip(workspace, writer, trigger_integration, conductor_api, network))]
pub async fn app_validation_workflow(
    mut workspace: AppValidationWorkspace,
    writer: OneshotWriter,
    trigger_integration: &mut TriggerSender,
    conductor_api: impl CellConductorApiT,
    network: HolochainP2pCell,
) -> WorkflowResult<WorkComplete> {
    let complete = app_validation_workflow_inner(&mut workspace, conductor_api, &network).await?;
    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer.with_writer(|writer| Ok(workspace.flush_to_txn(writer)?))?;

    // trigger other workflows
    trigger_integration.trigger();

    Ok(complete)
}
async fn app_validation_workflow_inner(
    workspace: &mut AppValidationWorkspace,
    conductor_api: impl CellConductorApiT,
    network: &HolochainP2pCell,
) -> WorkflowResult<WorkComplete> {
    let env = workspace.validation_limbo.env().clone();
    let (ops, mut awaiting_ops): (Vec<ValidationLimboValue>, Vec<ValidationLimboValue>) =
        fresh_reader!(env, |r| workspace
            .validation_limbo
            .drain_iter_filter(&r, |(_, vlv)| {
                match vlv.status {
                    // We only want sys validated or awaiting app dependency ops
                    ValidationLimboStatus::SysValidated
                    | ValidationLimboStatus::AwaitingAppDeps(_)
                    | ValidationLimboStatus::PendingValidation => Ok(true),
                    ValidationLimboStatus::Pending | ValidationLimboStatus::AwaitingSysDeps(_) => {
                        Ok(false)
                    }
                }
            })?
            // Partition awaiting proof into a separate vec
            .partition(|vlv| match vlv.status {
                ValidationLimboStatus::PendingValidation => Ok(false),
                _ => Ok(true),
            }))?;
    debug!(?ops, ?awaiting_ops);
    for mut vlv in ops {
        match &vlv.status {
            ValidationLimboStatus::AwaitingAppDeps(_) | ValidationLimboStatus::SysValidated => {
                let op = light_to_op(vlv.op.clone(), &workspace.element_pending).await?;

                // Validation
                let outcome = validate_op(
                    op.clone(),
                    &conductor_api,
                    workspace,
                    &network,
                    &mut vlv.pending_dependencies,
                )
                .await
                .or_else(|outcome_or_err| outcome_or_err.try_into())?;

                match outcome {
                    Outcome::Accepted => {
                        if vlv.pending_dependencies.pending_dependencies() {
                            vlv.status = ValidationLimboStatus::PendingValidation;
                            awaiting_ops.push(vlv);
                        } else {
                            let hash = DhtOpHash::with_data(&op).await;
                            let iv = IntegrationLimboValue {
                                validation_status: ValidationStatus::Valid,
                                op: vlv.op,
                            };
                            workspace.to_int_limbo(hash, iv, op)?;
                        }
                    }
                    Outcome::AwaitingDeps(deps) => {
                        let hash = DhtOpHash::with_data(&op).await;
                        vlv.status = ValidationLimboStatus::AwaitingAppDeps(deps);
                        workspace.to_val_limbo(hash, vlv)?;
                    }
                    Outcome::Rejected(_) => {
                        let hash = DhtOpHash::with_data(&op).await;
                        let iv = IntegrationLimboValue {
                            op: vlv.op,
                            validation_status: ValidationStatus::Rejected,
                        };
                        workspace.to_int_limbo(hash, iv, op)?;
                    }
                }
            }
            _ => unreachable!("Should not contain any other status"),
        }
    }
    fn check_dep_status(
        dep: &DhtOpHash,
        workspace: &AppValidationWorkspace,
    ) -> DatabaseResult<Option<ValidationStatus>> {
        let ilv = workspace.integration_limbo.get(dep)?;
        if let Some(ilv) = ilv {
            return Ok(Some(ilv.validation_status));
        }
        let iv = workspace.integrated_dht_ops.get(dep)?;
        if let Some(iv) = iv {
            return Ok(Some(iv.validation_status));
        }
        return Ok(None);
    }
    // Check awaiting proof that might be able to be progressed now.
    // Including any awaiting proof from this run.
    'op_loop: for mut vlv in awaiting_ops {
        let mut still_awaiting = Vec::new();
        for dep in vlv.pending_dependencies.pending.drain(..) {
            match check_dep_status(dep.as_ref(), &workspace)? {
                Some(status) => {
                    match status {
                        ValidationStatus::Valid => {
                            // Discarding dep because we have proof it's integrated and valid
                        }
                        ValidationStatus::Rejected | ValidationStatus::Abandoned => {
                            match dep {
                                DepType::FixedElement(_) => {
                                    // Mark this op as invalid and integrate it.
                                    // There is no reason to check the other deps as it is rejected.
                                    let op =
                                        light_to_op(vlv.op.clone(), &workspace.element_pending)
                                            .await?;
                                    let hash = DhtOpHash::with_data(&op).await;
                                    let iv = IntegrationLimboValue {
                                        validation_status: status,
                                        op: vlv.op,
                                    };
                                    workspace.to_int_limbo(hash, iv, op)?;

                                    // Continue to the next op
                                    continue 'op_loop;
                                }
                                DepType::AnyElement(_) => {
                                    // The dependency is any element with for an entry
                                    // So we can't say that it is invalid because there could
                                    // always be a valid entry.
                                    // TODO: Correctness: This probably has consequences beyond this
                                    // pr that we should come back to
                                }
                            }
                        }
                    }
                }
                None => {
                    // Dep is still not integrated so keep waiting
                    still_awaiting.push(dep);
                }
            }
        }
        let op = light_to_op(vlv.op.clone(), &workspace.element_pending).await?;
        let hash = DhtOpHash::with_data(&op).await;
        if still_awaiting.len() > 0 {
            vlv.pending_dependencies.pending = still_awaiting;
            workspace.to_val_limbo(hash, vlv)?;
        } else {
            let iv = IntegrationLimboValue {
                validation_status: ValidationStatus::Valid,
                op: vlv.op,
            };
            workspace.to_int_limbo(hash, iv, op)?;
        }
    }
    Ok(WorkComplete::Complete)
}

async fn validate_op(
    op: DhtOp,
    conductor_api: &impl CellConductorApiT,
    workspace: &mut AppValidationWorkspace,
    network: &HolochainP2pCell,
    dependencies: &mut PendingDependencies,
) -> AppValidationOutcome<Outcome> {
    use Either::*;
    // Create the element
    // TODO: remove clone of op
    let element = match get_element(op.clone()) {
        Left(el) => el,
        Right(o) => return Ok(o),
    };
    // Get the dna file
    let dna_file = { conductor_api.get_this_dna().await };
    let dna_file =
        dna_file.ok_or_else(|| AppValidationError::DnaMissing(conductor_api.cell_id().clone()))?;

    // Get the zome names
    let mut data_source = workspace.data_source(network);
    let zome_names = get_zome_names(&element, &dna_file, &mut data_source, dependencies).await?;
    // Create the ribosome
    let ribosome = WasmRibosome::new(dna_file);

    let outcome = match element.header() {
        Header::LinkAdd(link_add) => {
            let base = retrieve_entry(&link_add.base_address, &mut data_source)
                .await?
                .and_then(|dep| dependencies.store_entry_any(dep))
                .and_then(|e| e.into_inner().1)
                .ok_or_else(|| Outcome::awaiting(&link_add.base_address))?;
            let target = retrieve_entry(&link_add.target_address, &mut data_source)
                .await?
                .and_then(|dep| dependencies.store_entry_any(dep))
                .and_then(|e| e.into_inner().1)
                .ok_or_else(|| Outcome::awaiting(&link_add.target_address))?;

            let link_add = Arc::new(link_add.clone());
            let base = Arc::new(base);
            let target = Arc::new(target);
            zome_names
                .into_iter()
                .map(|zome_name| {
                    run_link_validation_callback(
                        zome_name,
                        link_add.clone(),
                        base.clone(),
                        target.clone(),
                        &ribosome,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .find(|o| match o {
                    Outcome::AwaitingDeps(_) | Outcome::Rejected(_) => true,
                    Outcome::Accepted => false,
                })
                .unwrap_or(Outcome::Accepted)
        }
        _ => {
            // Entry

            // Call the callback
            // TODO: Not sure if this is correct? Calling every zome
            // for an agent key etc. If so we should change run_validation
            // to a Vec<ZomeName>
            let element = Arc::new(element);
            zome_names
                .into_iter()
                .map(|zome_name| run_validation_callback(zome_name, element.clone(), &ribosome))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .find(|o| match o {
                    Outcome::AwaitingDeps(_) | Outcome::Rejected(_) => true,
                    Outcome::Accepted => false,
                })
                .unwrap_or(Outcome::Accepted)
        }
    };
    if let Outcome::AwaitingDeps(_) | Outcome::Rejected(_) = &outcome {
        warn!(
            agent = %which_agent(conductor_api.cell_id().agent_pubkey()),
            msg = "DhtOp has failed app validation",
            ?op,
            outcome = ?outcome,
        );
    }

    Ok(outcome)
}

fn get_element(op: DhtOp) -> Either<Element, Outcome> {
    use Either::*;
    match op {
        DhtOp::RegisterDeletedBy(_, _) | DhtOp::RegisterAgentActivity(_, _) => {
            Right(Outcome::Accepted)
        }
        DhtOp::StoreElement(_, h, _) => match h {
            Header::ElementDelete(_) => todo!("Get the original entry"),
            _ => Right(Outcome::Accepted),
        },
        DhtOp::StoreEntry(s, h, e) => Left(Element::new(
            SignedHeaderHashed::with_presigned(HeaderHashed::from_content_sync(h.into()), s),
            Some(*e),
        )),
        DhtOp::RegisterUpdatedBy(s, h) => Left(Element::new(
            SignedHeaderHashed::with_presigned(HeaderHashed::from_content_sync(h.into()), s),
            None,
        )),
        DhtOp::RegisterDeletedEntryHeader(s, h) => Left(Element::new(
            SignedHeaderHashed::with_presigned(HeaderHashed::from_content_sync(h.into()), s),
            None,
        )),
        DhtOp::RegisterAddLink(s, h) => Left(Element::new(
            SignedHeaderHashed::with_presigned(HeaderHashed::from_content_sync(h.into()), s),
            None,
        )),
        DhtOp::RegisterRemoveLink(s, h) => Left(Element::new(
            SignedHeaderHashed::with_presigned(HeaderHashed::from_content_sync(h.into()), s),
            None,
        )),
    }
}

async fn get_app_entry_type(
    element: &Element,
    data_source: &mut AppValDataSource<'_>,
    dependencies: &mut PendingDependencies,
) -> AppValidationOutcome<Option<AppEntryType>> {
    match element.header().entry_data() {
        Some((_, et)) => match et.clone() {
            EntryType::App(aet) => Ok(Some(aet)),
            EntryType::AgentPubKey | EntryType::CapClaim | EntryType::CapGrant => Ok(None),
        },
        None => get_app_entry_type_from_dep(element, data_source, dependencies).await,
    }
}

async fn get_app_entry_type_from_dep(
    element: &Element,
    data_source: &mut AppValDataSource<'_>,
    dependencies: &mut PendingDependencies,
) -> AppValidationOutcome<Option<AppEntryType>> {
    match element.header() {
        Header::LinkAdd(la) => {
            let el = retrieve_entry(&la.base_address, data_source)
                .await?
                .and_then(|dep| dependencies.store_entry_any(dep))
                .ok_or_else(|| Outcome::awaiting(&la.base_address))?;
            Ok(extract_app_type(&el))
        }
        Header::LinkRemove(lr) => {
            let el = retrieve_entry(&lr.base_address, data_source)
                .await?
                .and_then(|dep| dependencies.store_entry_any(dep))
                .ok_or_else(|| Outcome::awaiting(&lr.base_address))?;
            Ok(extract_app_type(&el))
        }
        Header::EntryUpdate(eu) => {
            let el = retrieve_element(&eu.original_header_address, data_source)
                .await?
                .and_then(|dep| dependencies.store_entry_fixed(dep))
                .ok_or_else(|| Outcome::awaiting(&eu.original_header_address))?;
            Ok(extract_app_type(&el))
        }
        Header::ElementDelete(ed) => {
            let el = retrieve_element(&ed.removes_address, data_source)
                .await?
                .and_then(|dep| dependencies.store_entry_fixed(dep))
                .ok_or_else(|| Outcome::awaiting(&ed.removes_address))?;
            Ok(extract_app_type(&el))
        }
        _ => todo!(),
    }
}

fn extract_app_type(element: &Element) -> Option<AppEntryType> {
    element
        .header()
        .entry_data()
        .and_then(|(_, entry_type)| match entry_type {
            EntryType::App(aet) => Some(aet.clone()),
            _ => None,
        })
}

async fn get_zome_names(
    element: &Element,
    dna_file: &DnaFile,
    data_source: &mut AppValDataSource<'_>,
    dependencies: &mut PendingDependencies,
) -> AppValidationOutcome<Vec<ZomeName>> {
    match get_app_entry_type(element, data_source, dependencies).await? {
        Some(aet) => Ok(vec![get_zome_name(&aet, &dna_file)?]),
        None => Ok(dna_file
            .dna()
            .zomes
            .iter()
            .map(|(z, _)| z.clone())
            .collect()),
    }
}

fn get_zome_name(entry_type: &AppEntryType, dna_file: &DnaFile) -> AppValidationResult<ZomeName> {
    let zome_index = u8::from(entry_type.zome_id()) as usize;
    Ok(dna_file
        .dna()
        .zomes
        .get(zome_index)
        .ok_or_else(|| AppValidationError::ZomeId(entry_type.clone()))?
        .0
        .clone())
}

fn run_validation_callback(
    zome_name: ZomeName,
    element: Arc<Element>,
    ribosome: &impl RibosomeT,
) -> AppValidationResult<Outcome> {
    let validate: ValidateResult = ribosome.run_validate(
        ValidateHostAccess,
        ValidateInvocation { zome_name, element },
    )?;
    match validate {
        ValidateResult::Valid => Ok(Outcome::Accepted),
        ValidateResult::Invalid(reason) => Ok(Outcome::Rejected(reason)),
        ValidateResult::UnresolvedDependencies(hashes) => {
            let deps = hashes.into_iter().map(AnyDhtHash::from).collect();
            Ok(Outcome::AwaitingDeps(deps))
        }
    }
}

fn run_link_validation_callback(
    zome_name: ZomeName,
    link_add: Arc<LinkAdd>,
    base: Arc<Entry>,
    target: Arc<Entry>,
    ribosome: &impl RibosomeT,
) -> AppValidationResult<Outcome> {
    let invocation = ValidateLinkAddInvocation {
        zome_name,
        link_add,
        base,
        target,
    };
    let validate = ribosome.run_validate_link_add(ValidateLinkAddHostAccess, invocation)?;
    match validate {
        ValidateLinkAddResult::Valid => Ok(Outcome::Accepted),
        ValidateLinkAddResult::Invalid(reason) => Ok(Outcome::Rejected(reason)),
    }
}

pub struct AppValidationWorkspace {
    pub integrated_dht_ops: IntegratedDhtOpsStore,
    pub integration_limbo: IntegrationLimboStore,
    pub validation_limbo: ValidationLimboStore,
    // Integrated data
    pub element_vault: ElementBuf,
    pub meta_vault: MetadataBuf,
    // Data pending validation
    pub element_pending: ElementBuf<PendingPrefix>,
    pub meta_pending: MetadataBuf<PendingPrefix>,
    // Data that has progressed past validation and is pending Integration
    pub element_judged: ElementBuf<JudgedPrefix>,
    pub meta_judged: MetadataBuf<JudgedPrefix>,
    // Cached data
    pub element_cache: ElementBuf,
    pub meta_cache: MetadataBuf,
    // Ops to disintegrate
    pub to_disintegrate_pending: Vec<DhtOpLight>,
}

impl AppValidationWorkspace {
    pub fn new(env: EnvironmentRead) -> WorkspaceResult<Self> {
        let db = env.get_db(&*INTEGRATED_DHT_OPS)?;
        let integrated_dht_ops = KvBufFresh::new(env.clone(), db);
        let db = env.get_db(&*INTEGRATION_LIMBO)?;
        let integration_limbo = KvBufFresh::new(env.clone(), db);

        let validation_limbo = ValidationLimboStore::new(env.clone())?;

        let element_vault = ElementBuf::vault(env.clone(), false)?;
        let meta_vault = MetadataBuf::vault(env.clone())?;
        let element_cache = ElementBuf::cache(env.clone())?;
        let meta_cache = MetadataBuf::cache(env.clone())?;

        let element_pending = ElementBuf::pending(env.clone())?;
        let meta_pending = MetadataBuf::pending(env.clone())?;

        let element_judged = ElementBuf::judged(env.clone())?;
        let meta_judged = MetadataBuf::judged(env)?;

        Ok(Self {
            integrated_dht_ops,
            integration_limbo,
            validation_limbo,
            element_vault,
            meta_vault,
            element_pending,
            meta_pending,
            element_judged,
            meta_judged,
            element_cache,
            meta_cache,
            to_disintegrate_pending: Vec::new(),
        })
    }

    fn data_source<'a>(&'a mut self, network: &'a HolochainP2pCell) -> AppValDataSource<'a> {
        AppValDataSource {
            workspace: self,
            network,
        }
    }

    fn to_val_limbo(
        &mut self,
        hash: DhtOpHash,
        mut vlv: ValidationLimboValue,
    ) -> WorkflowResult<()> {
        vlv.last_try = Some(Timestamp::now());
        vlv.num_tries += 1;
        self.validation_limbo.put(hash, vlv)?;
        Ok(())
    }

    #[tracing::instrument(skip(self, hash))]
    fn to_int_limbo(
        &mut self,
        hash: DhtOpHash,
        iv: IntegrationLimboValue,
        op: DhtOp,
    ) -> WorkflowResult<()> {
        disintegrate_single_metadata(iv.op.clone(), &self.element_pending, &mut self.meta_pending)?;
        self.to_disintegrate_pending.push(iv.op.clone());
        integrate_single_data(op, &mut self.element_judged)?;
        integrate_single_metadata(iv.op.clone(), &self.element_judged, &mut self.meta_judged)?;
        self.integration_limbo.put(hash, iv)?;
        Ok(())
    }

    #[tracing::instrument(skip(self, writer))]
    /// We need to cancel any deletes for the pending data
    /// where the ops still in validation limbo reference that data
    fn update_element_stores(&mut self, writer: &mut Writer) -> WorkspaceResult<()> {
        for op in self.to_disintegrate_pending.drain(..) {
            disintegrate_single_data(op, &mut self.element_pending);
        }
        let mut val_iter = self.validation_limbo.iter(writer)?;
        while let Some((_, vlv)) = val_iter.next()? {
            reintegrate_single_data(vlv.op, &mut self.element_pending);
        }
        Ok(())
    }
}

impl Workspace for AppValidationWorkspace {
    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> WorkspaceResult<()> {
        self.update_element_stores(writer)?;
        self.validation_limbo.0.flush_to_txn_ref(writer)?;
        self.integration_limbo.flush_to_txn_ref(writer)?;
        self.element_pending.flush_to_txn_ref(writer)?;
        self.meta_pending.flush_to_txn_ref(writer)?;
        self.element_judged.flush_to_txn_ref(writer)?;
        self.meta_judged.flush_to_txn_ref(writer)?;
        Ok(())
    }
}
struct AppValDataSource<'a> {
    workspace: &'a mut AppValidationWorkspace,
    network: &'a HolochainP2pCell,
}

impl DataSource for AppValDataSource<'_> {
    fn cascade(&mut self) -> Cascade {
        let workspace = &mut self.workspace;
        Cascade::new(
            workspace.validation_limbo.env().clone(),
            &workspace.element_vault,
            &workspace.meta_vault,
            &mut workspace.element_cache,
            &mut workspace.meta_cache,
            self.network.clone(),
        )
    }

    fn pending(&self) -> DbPair<PendingPrefix, MetadataBuf<PendingPrefix>> {
        DbPair {
            element: &self.workspace.element_pending,
            meta: &self.workspace.meta_pending,
        }
    }

    fn judged(&self) -> DbPair<JudgedPrefix, MetadataBuf<JudgedPrefix>> {
        DbPair {
            element: &self.workspace.element_judged,
            meta: &self.workspace.meta_judged,
        }
    }
}
