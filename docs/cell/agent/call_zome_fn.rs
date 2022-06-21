/// Called from Conductor:
///
/// - Conductor receiving a call_zome request (TODO: tracing?)
/// -
/// - Initialization of zomes
///
/// - Receive callback
///
///
///
/// Parameters (expected types/structures):
///
/// ZomeCall Params
/// As-At (action_seq that is equal to chain_head of Source Chain at the time of initiating the Call ZomeFn workflow)
/// Provenance
/// Capability token secret (if this is not calling a Public trait function)
/// Cell Context (also handle to keystore)
///
///
/// Data X (data & structure) from Store Y:
///
/// - Get source chain head as our "as at"
/// - Private entry CAS to look up for the Capability secret we have a parameter
///
///
///
/// Functions / Workflows:
///
/// 1.  Check if there is a Capability token secret in the parameters. If there isn't and the function to be called isn't public, we stop the process and return an error.
///
///   1.1 If there is a secret, we look up our private CAS and see if it matches any secret for a Capability Grant entry that we have stored. If it does, check that this Capability Grant is not revoked and actually grants permissions to call the ZomeFn that is being called.
///
///   1.2 Check if the Capability Grant has assignees=None (means this Capability is transferable). If it has assignees=Vec<Address> (means this Capability is on Assigned mode, check that the provenance's agent key is in that assignees.
///
///   1.3 If the CapabiltyGrant has pre-filled parameters, check that the ui is passing exactly the parameters needed and no more to complete the call.
///
/// 2. Set Context (Cascading Cursor w/ Pre-flight chain extension)
///
/// 3. Invoke WASM (w/ Cursor)
///
/// WASM receives external call handles: (gets & commits via cascading cursor, crypto functions & bridge calls via conductor, send via network function call for send direct message)
/// 4. When the WASM code execution finishes, If workspace has new chain entries:
///
///   4.1. Call system validation of list of entries and actions:
///
/// Check entry hash
/// Check action hash
/// Check action signature
/// Check action timestamp is later than previous timestamp
/// Check entry content matches entry schema
/// Depending on the type of the commit, validate all possible validations for the DHT Op that would be produced by it
///   4.2. Call app validation of list of entries and actions:
///
/// Call validate_set_of_entries_and_actions (any necessary get results where we receive None / Timeout on retrieving validation dependencies, should produce error/fail)
///   4.3. Write output results via SC gatekeeper (wrap in transaction):
///
/// Get write handle to Source Chain
/// Check if chain_head === 'as-at'. If it is not, fail the whole process. It is important that we read after we have opened the write handle since this will lock the handle and we'll avoid race conditions.
/// Write the new Entries and Actions into the CAS.
/// Write the new Entries and Actions CRUDstatus=Live to CAS-Meta.
/// Write the new Actions records on Source Chain, with dht_transforms_completed=false.
/// Store CRUDstatus=Live in CAS-meta
/// Write the new chain_head on Source Chain.
///
///
/// 5. Return WASM Result & Destroy temp workspace
///
///
///
/// Persisted X Changes to Store Y (data & structure):
///
/// New Actions to Source Chain
/// New Chain head to Source Chain
/// New Actions and Entries to CAS
/// Store CRUDstatus=Live in CAS-meta
///
///
/// Returned Results (type & structure):
///
/// Return WASM result to the caller
///
///
/// Triggers:
///
/// Publish to DHT (Public Chain Entries, Actions)
