        /// Called from (w/ tracing context = child/follow):
///
/// - Initialization of zomes / trace=?
/// - Conductor receiving a call_zome request / trace=child
/// - Bridge calls from other Zomes / trace=child
/// - Receive Direct Message / trace=?
/// - Zome callback functions (validation, etc.) / trace=?
///

use crate::Workflow;

/// Parameters (expected types/structures):
///
/// ZomeCall Params
/// As-At (header_seq that is equal to chain_head of Source Chain at the time of initiating the Call ZomeFn workflow)
/// Provenance
/// Capability token secret (if this is not calling a Public trait function)
/// Cell Context (also handle to keystore)
///



pub struct ZomeCallParams {
    provenance: Provenance,
    capability_token: Option<Token>,
    fn_name: String,
    fn_params: HashMap<String, JsonString???>
}
pub struct ZomeCallFn {
    params: ZomeCallParams,
    conductor_api: ConductorApi,
    state: HolochainState,
}


/// Functions / Workflows:

/// Starts the Workflow
impl ZomeCallFn {
    pub fn new(params: ZomeCallParams, cell_context: CellContext, state: HolochainState) -> Self {
        ZomeCallFn {
            params, conductor_api: cell_context.get_conductor_api(), state
        }
    }
}

/// Contains all the functions to call in the workflows
///
impl ZomeCallFn for Workflow {
    /// Data X (data & structure) from Store Y:
    ///
    /// - Private entry CAS to look up for the Capability secret we have a parameter
    /// - Get HEAD from source_chain. Set "as_at" context

    pub fn start(self) -> Future<()> {
        /// 1. Check if there is a Capability token secret in the parameters. If there isn't and the
        ///    function to be called isn't public, we stop the process and return an error.
        self.check_capability()?;

        /// 2. Set Context (Cascading Cursor w/ Pre-flight chain extension)
        let as_at = self.state.get("SOURCE_CHAIN", "chain_head");

         /// 3. Invoke WASM (w/ Cursor)
        let input: { fn_name, params_name, other_deps... } = host_args!(host_allocation_ptr);
        let processed: SomeStruct = host_call!(__test_process_struct, input);
        ret!(processed);

        /// WASM needs access to holochain_state, conductor_api, and ability to send_direct_message

        /// 4. When the WASM code execution finishes, If workspace is empty CALL FINISHER

        /// If workspace has new chain entries, call selective validation functions from the validation workflows
        if state.preflight.entries.len() > 0 {
            let dht_ops = dht::produce_dht_op_transforms(state.preflight.entries);
            sys_validate_preflight_chain_entries(dht_ops, state)?;
            app_validate_preflight_chain_entries(dht_ops, state)?;
        }

        self.finish(as_at)
    }

    fn finish(self, as_at: Address) -> Result<()> {
        let tx = self.state.begin_rw_transaction();

        let current_as_at = self.state.get("SOURCE_CHAIN", "chain_head");

        if current_as_at != as_at {
            return Err(String::from("Cannot commit when other CallZomeFn workflows have advanced the source chain");
        }

        tx.push(self.state.store("CAS", self.state.preflight.entries));
        tx.push(self.state.store("CAS-Meta", self.state.preflight.entries, CRUDStatus=Live));
        tx.push(self.state.store("SourceChain", self.state.preflight.entries, dht_op_transforms=false));
        tx.push(self.state.store("SourceChain", "chain_head", as_at));

        tx.commit();
    }
}

impl ZomeCallFn {
        /// Check capability token to confirm that the inbound call has permissions BEFORE invoking WASM.
        ///
    fn check_capability(self) -> Result<()> {
        /// Check if there is a Capability token secret in the parameters. If there isn't and the
        /// function to be called isn't public, we stop the process and return an error.
        ///   1.1 If there is a secret, we look up our private CAS and see if it matches any secret for a
        ///       Capability Grant entry that we have stored. If it does, check that this Capability Grant
        ///       is not revoked and actually grants permissions to call the ZomeFn that is being called.


        ///   1.2 Check if the Capability Grant has assignees=None (means this Capability is
        ///       transferable). If it has assignees=Vec<Address> (means this Capability is on Assigned mode, check that the provenance's agent key is in that assignees.
        ///
        ///   1.3 If the CapabiltyGrant has pre-filled parameters, check that the ui is passing exactly the parameters needed and no more to complete the call, else if params do not match curried constraints, overwrite passed values with curried values specified in grant.

    }

    fn sys_validate_preflight_chain_entries(dht_ops: Vec<DHTOp>, state: HolochainState) -> Result<()> {
        for dht_op in dht_ops {
            validation::check_entry_hash(entry, header)?;
        }
    }
    fn app_validate_preflight_chain_entries(dht_ops: DHTOp, state: HolochainState) -> Result<()> {
        for dht_op in dht_ops {
            validation::check_entry_hash(entry, header)?;
        }
    }
}

///
///
///
///
/// WASM receives external call handles: (gets & commits via cascading cursor, crypto functions & bridge calls via conductor, send via network function call for send direct message)

/// 4. When the WASM code execution finishes, If workspace has new chain entries:
///
///   4.1. Call system validation of list of entries and headers:
///
/// Check entry hash
/// Check header hash
/// Check header signature
/// Check header timestamp is later than previous timestamp
/// Check entry content matches entry schema
/// Depending on the type of the commit, validate all possible validations for the DHT Op that would be produced by it
///   4.2. Call app validation of list of entries and headers:
///
/// Call validate_set_of_entries_and_headers (any necessary get results where we receive None / Timeout on retrieving validation dependencies, should produce error/fail)


/// 5. FINISHER
///    Write output results via SC gatekeeper (wrap in transaction):
///
/// Get write handle to Source Chain
/// Check if chain_head === 'as-at'. If it is not, fail the whole process. It is important that we read after we have opened the write handle since this will lock the handle and we'll avoid race conditions.
/// Write the new Entries and Headers into the CAS.
/// Write the new Entries and Headers CRUDstatus=Live to CAS-Meta.
/// Write the new Headers records on Source Chain, with dht_transforms_completed=false.
/// Store CRUDstatus=Live in CAS-meta
/// Write the new chain_head on Source Chain.
///
///
///
/// Persist X Changes to Store Y (data & structure):
///
/// New Headers to Source Chain
/// New Chain head to Source Chain
/// New Headers and Entries to CAS
/// Store CRUDstatus=Live in CAS-meta
///
/// Triggers:
///
/// Publish to DHT (Public Chain Entries, Headers)
///
///
/// Returned Results (type & structure):
///
/// Return WASM result to the caller
///  Return WASM Result & Destroy temp workspace
///


#[cfg(test)]
mod tests {

  /// Fixtures:
  /// Initial Holochain-state needed to start the workflow for the lmdb mock

  let holochain_state = r#
{
  "comment":"output from lmdb state that should generate this fixture goes here"
}
#
  // or
  let holochain_state = get_state_fixture_for_test("call zome test"); // returns a HolochainState

  /// Worflow input fixtures
  let workflow_params = ZomeCallParams {
    provenance: Provenance,
    capability_token: Option<Token>,
    fn_name: String,
    fn_params: HashMap<String, JsonString???>
  }

  let workflow_cell_context = make_cell_context_for_test();; returns a CellContext

  /// test start
  #[test]
  fn test_call_zome_workflow_start() {
  }

  /// test finish
  #[test]
  fn test_call_zome_workflow_finish() {
  }


}
