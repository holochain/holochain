use crate::prelude::*;
use holochain_deterministic_integrity::hdi::HdiT;

pub const HDK_NOT_REGISTERED: &str = "HDK not registered";

/// This is a cell so it can be set many times.
/// Every test needs its own mock so each test needs to set it.
use core::cell::RefCell;
use std::rc::Rc;

#[cfg(any(feature = "mock", not(target_arch = "wasm32")))]
thread_local!(pub static HDK: RefCell<Rc<dyn HdkT>> = RefCell::new(Rc::new(ErrHdk)));

#[cfg(all(not(feature = "mock"), target_arch = "wasm32"))]
thread_local!(pub static HDK: RefCell<Rc<dyn HdkT>> = RefCell::new(Rc::new(HostHdk)));

/// When mocking is enabled the mockall crate automatically builds a MockHdkT for us.
/// ```ignore
/// let mut mock = MockHdkT::new();
/// mock_hdk.expect_foo().times(1).etc().etc();
/// set_hdk(mock_hdk);
/// ```
pub trait HdkT: HdiT {
    // Chain
    fn get_agent_activity(
        &self,
        get_agent_activity_input: GetAgentActivityInput,
    ) -> ExternResult<AgentActivity>;
    fn query(&self, filter: ChainQueryFilter) -> ExternResult<Vec<Element>>;
    // Ed25519
    fn sign(&self, sign: Sign) -> ExternResult<Signature>;
    fn sign_ephemeral(&self, sign_ephemeral: SignEphemeral) -> ExternResult<EphemeralSignatures>;
    // Entry
    fn create(&self, create_input: CreateInput) -> ExternResult<ActionHash>;
    fn update(&self, update_input: UpdateInput) -> ExternResult<ActionHash>;
    fn delete(&self, delete_input: DeleteInput) -> ExternResult<ActionHash>;
    fn get(&self, get_input: Vec<GetInput>) -> ExternResult<Vec<Option<Element>>>;
    fn get_details(&self, get_input: Vec<GetInput>) -> ExternResult<Vec<Option<Details>>>;
    // CounterSigning
    fn accept_countersigning_preflight_request(
        &self,
        preflight_request: PreflightRequest,
    ) -> ExternResult<PreflightRequestAcceptance>;
    // Info
    fn agent_info(&self, agent_info_input: ()) -> ExternResult<AgentInfo>;
    fn call_info(&self, call_info_input: ()) -> ExternResult<CallInfo>;
    // Link
    fn create_link(&self, create_link_input: CreateLinkInput) -> ExternResult<ActionHash>;
    fn delete_link(&self, delete_link_input: DeleteLinkInput) -> ExternResult<ActionHash>;
    fn get_links(&self, get_links_input: Vec<GetLinksInput>) -> ExternResult<Vec<Vec<Link>>>;
    fn get_link_details(
        &self,
        get_links_input: Vec<GetLinksInput>,
    ) -> ExternResult<Vec<LinkDetails>>;
    // P2P
    fn call(&self, call: Vec<Call>) -> ExternResult<Vec<ZomeCallResponse>>;
    fn emit_signal(&self, app_signal: AppSignal) -> ExternResult<()>;
    fn remote_signal(&self, remote_signal: RemoteSignal) -> ExternResult<()>;
    // Random
    fn random_bytes(&self, number_of_bytes: u32) -> ExternResult<Bytes>;
    // Time
    fn sys_time(&self, sys_time_input: ()) -> ExternResult<Timestamp>;
    fn schedule(&self, scheduled_fn: String) -> ExternResult<()>;
    fn sleep(&self, wake_after: std::time::Duration) -> ExternResult<()>;
    // XSalsa20Poly1305
    fn x_salsa20_poly1305_shared_secret_create_random(
        &self,
        key_ref: Option<XSalsa20Poly1305KeyRef>,
    ) -> ExternResult<XSalsa20Poly1305KeyRef>;
    fn x_salsa20_poly1305_shared_secret_export(
        &self,
        x_salsa20_poly1305_shared_secret_export: XSalsa20Poly1305SharedSecretExport,
    ) -> ExternResult<XSalsa20Poly1305EncryptedData>;
    fn x_salsa20_poly1305_shared_secret_ingest(
        &self,
        x_salsa20_poly1305_shared_secret_ingest: XSalsa20Poly1305SharedSecretIngest,
    ) -> ExternResult<XSalsa20Poly1305KeyRef>;
    fn x_salsa20_poly1305_encrypt(
        &self,
        x_salsa20_poly1305_encrypt: XSalsa20Poly1305Encrypt,
    ) -> ExternResult<XSalsa20Poly1305EncryptedData>;
    fn create_x25519_keypair(&self, create_x25519_keypair_input: ()) -> ExternResult<X25519PubKey>;
    fn x_25519_x_salsa20_poly1305_encrypt(
        &self,
        x_25519_x_salsa20_poly1305_encrypt: X25519XSalsa20Poly1305Encrypt,
    ) -> ExternResult<XSalsa20Poly1305EncryptedData>;
}

#[cfg(feature = "mock")]
mockall::mock! {
    pub HdkT {}

    impl HdkT for HdkT {
        // Chain
        fn get_agent_activity(
            &self,
            get_agent_activity_input: GetAgentActivityInput,
        ) -> ExternResult<AgentActivity>;
        fn query(&self, filter: ChainQueryFilter) -> ExternResult<Vec<Element>>;
        // Ed25519
        fn sign(&self, sign: Sign) -> ExternResult<Signature>;
        fn sign_ephemeral(&self, sign_ephemeral: SignEphemeral) -> ExternResult<EphemeralSignatures>;
        // Entry
        fn create(&self, create_input: CreateInput) -> ExternResult<ActionHash>;
        fn update(&self, update_input: UpdateInput) -> ExternResult<ActionHash>;
        fn delete(&self, delete_input: DeleteInput) -> ExternResult<ActionHash>;
        fn get(&self, get_input: Vec<GetInput>) -> ExternResult<Vec<Option<Element>>>;
        fn get_details(&self, get_input: Vec<GetInput>) -> ExternResult<Vec<Option<Details>>>;
        // CounterSigning
        fn accept_countersigning_preflight_request(
            &self,
            preflight_request: PreflightRequest,
        ) -> ExternResult<PreflightRequestAcceptance>;
        // Info
        fn agent_info(&self, agent_info_input: ()) -> ExternResult<AgentInfo>;
        fn call_info(&self, call_info_input: ()) -> ExternResult<CallInfo>;
        // Link
        fn create_link(&self, create_link_input: CreateLinkInput) -> ExternResult<ActionHash>;
        fn delete_link(&self, delete_link_input: DeleteLinkInput) -> ExternResult<ActionHash>;
        fn get_links(&self, get_links_input: Vec<GetLinksInput>) -> ExternResult<Vec<Vec<Link>>>;
        fn get_link_details(
            &self,
            get_links_input: Vec<GetLinksInput>,
        ) -> ExternResult<Vec<LinkDetails>>;
        // P2P
        fn call(&self, call: Vec<Call>) -> ExternResult<Vec<ZomeCallResponse>>;
        fn emit_signal(&self, app_signal: AppSignal) -> ExternResult<()>;
        fn remote_signal(&self, remote_signal: RemoteSignal) -> ExternResult<()>;
        // Random
        fn random_bytes(&self, number_of_bytes: u32) -> ExternResult<Bytes>;
        // Time
        fn sys_time(&self, sys_time_input: ()) -> ExternResult<Timestamp>;
        fn schedule(&self, scheduled_fn: String) -> ExternResult<()>;
        fn sleep(&self, wake_after: std::time::Duration) -> ExternResult<()>;
        // XSalsa20Poly1305
        fn x_salsa20_poly1305_shared_secret_create_random(
            &self,
            key_ref: Option<XSalsa20Poly1305KeyRef>,
        ) -> ExternResult<XSalsa20Poly1305KeyRef>;
        fn x_salsa20_poly1305_shared_secret_export(
            &self,
            x_salsa20_poly1305_shared_secret_export: XSalsa20Poly1305SharedSecretExport,
        ) -> ExternResult<XSalsa20Poly1305EncryptedData>;
        fn x_salsa20_poly1305_shared_secret_ingest(
            &self,
            x_salsa20_poly1305_shared_secret_ingest: XSalsa20Poly1305SharedSecretIngest,
        ) -> ExternResult<XSalsa20Poly1305KeyRef>;
        fn x_salsa20_poly1305_encrypt(
            &self,
            x_salsa20_poly1305_encrypt: XSalsa20Poly1305Encrypt,
        ) -> ExternResult<XSalsa20Poly1305EncryptedData>;
        fn create_x25519_keypair(&self, create_x25519_keypair_input: ()) -> ExternResult<X25519PubKey>;
        fn x_25519_x_salsa20_poly1305_encrypt(
            &self,
            x_25519_x_salsa20_poly1305_encrypt: X25519XSalsa20Poly1305Encrypt,
        ) -> ExternResult<XSalsa20Poly1305EncryptedData>;

    }

    impl HdiT for HdkT {
        fn verify_signature(&self, verify_signature: VerifySignature) -> ExternResult<bool>;
        fn hash(&self, hash_input: HashInput) -> ExternResult<HashOutput>;
        fn must_get_entry(&self, must_get_entry_input: MustGetEntryInput) -> ExternResult<EntryHashed>;
        fn must_get_action(
            &self,
            must_get_action_input: MustGetActionInput,
        ) -> ExternResult<SignedActionHashed>;
        fn must_get_valid_element(
            &self,
            must_get_valid_element_input: MustGetValidElementInput,
        ) -> ExternResult<Element>;
        // Info
        fn dna_info(&self, dna_info_input: ()) -> ExternResult<DnaInfo>;
        fn zome_info(&self, zome_info_input: ()) -> ExternResult<ZomeInfo>;
        // Trace
        fn trace(&self, trace_msg: TraceMsg) -> ExternResult<()>;
        // XSalsa20Poly1305
        fn x_salsa20_poly1305_decrypt(
            &self,
            x_salsa20_poly1305_decrypt: XSalsa20Poly1305Decrypt,
        ) -> ExternResult<Option<XSalsa20Poly1305Data>>;
        fn x_25519_x_salsa20_poly1305_decrypt(
            &self,
            x_25519_x_salsa20_poly1305_decrypt: X25519XSalsa20Poly1305Decrypt,
        ) -> ExternResult<Option<XSalsa20Poly1305Data>>;
    }

}

/// Used as a placeholder before any other Hdk is registered.
/// Generally only useful for testing but technically can be set any time.
pub struct ErrHdk;

impl ErrHdk {
    fn err<T>() -> ExternResult<T> {
        Err(wasm_error!(WasmErrorInner::Guest(
            HDK_NOT_REGISTERED.to_string()
        )))
    }
}

/// Every call is an error for the ErrHdk.
impl HdiT for ErrHdk {
    fn verify_signature(&self, _verify_signature: VerifySignature) -> ExternResult<bool> {
        Self::err()
    }

    fn hash(&self, _hash_input: HashInput) -> ExternResult<HashOutput> {
        Self::err()
    }

    fn must_get_entry(
        &self,
        _must_get_entry_input: MustGetEntryInput,
    ) -> ExternResult<EntryHashed> {
        Self::err()
    }

    fn must_get_action(
        &self,
        _must_get_action_input: MustGetActionInput,
    ) -> ExternResult<SignedActionHashed> {
        Self::err()
    }

    fn must_get_valid_element(
        &self,
        _must_get_valid_element_input: MustGetValidElementInput,
    ) -> ExternResult<Element> {
        Self::err()
    }

    fn dna_info(&self, _dna_info_input: ()) -> ExternResult<DnaInfo> {
        Self::err()
    }

    fn zome_info(&self, _zome_info_input: ()) -> ExternResult<ZomeInfo> {
        Self::err()
    }

    fn trace(&self, _: TraceMsg) -> ExternResult<()> {
        Self::err()
    }

    fn x_salsa20_poly1305_decrypt(
        &self,
        _x_salsa20_poly1305_decrypt: XSalsa20Poly1305Decrypt,
    ) -> ExternResult<Option<XSalsa20Poly1305Data>> {
        Self::err()
    }

    fn x_25519_x_salsa20_poly1305_decrypt(
        &self,
        _x_25519_x_salsa20_poly1305_decrypt: X25519XSalsa20Poly1305Decrypt,
    ) -> ExternResult<Option<XSalsa20Poly1305Data>> {
        Self::err()
    }
}

/// Every call is an error for the ErrHdk.
impl HdkT for ErrHdk {
    fn get_agent_activity(&self, _: GetAgentActivityInput) -> ExternResult<AgentActivity> {
        Self::err()
    }
    fn query(&self, _: ChainQueryFilter) -> ExternResult<Vec<Element>> {
        Self::err()
    }
    fn sign(&self, _: Sign) -> ExternResult<Signature> {
        Self::err()
    }
    fn sign_ephemeral(&self, _: SignEphemeral) -> ExternResult<EphemeralSignatures> {
        Self::err()
    }
    fn create(&self, _: CreateInput) -> ExternResult<ActionHash> {
        Self::err()
    }
    fn update(&self, _: UpdateInput) -> ExternResult<ActionHash> {
        Self::err()
    }
    fn delete(&self, _: DeleteInput) -> ExternResult<ActionHash> {
        Self::err()
    }
    fn get(&self, _: Vec<GetInput>) -> ExternResult<Vec<Option<Element>>> {
        Self::err()
    }
    fn get_details(&self, _: Vec<GetInput>) -> ExternResult<Vec<Option<Details>>> {
        Self::err()
    }
    // CounterSigning
    fn accept_countersigning_preflight_request(
        &self,
        _: PreflightRequest,
    ) -> ExternResult<PreflightRequestAcceptance> {
        Self::err()
    }
    fn agent_info(&self, _: ()) -> ExternResult<AgentInfo> {
        Self::err()
    }
    fn call_info(&self, _: ()) -> ExternResult<CallInfo> {
        Self::err()
    }
    // Link
    fn create_link(&self, _: CreateLinkInput) -> ExternResult<ActionHash> {
        Self::err()
    }
    fn delete_link(&self, _: DeleteLinkInput) -> ExternResult<ActionHash> {
        Self::err()
    }
    fn get_links(&self, _: Vec<GetLinksInput>) -> ExternResult<Vec<Vec<Link>>> {
        Self::err()
    }
    fn get_link_details(&self, _: Vec<GetLinksInput>) -> ExternResult<Vec<LinkDetails>> {
        Self::err()
    }
    // P2P
    fn call(&self, _: Vec<Call>) -> ExternResult<Vec<ZomeCallResponse>> {
        Self::err()
    }
    fn emit_signal(&self, _: AppSignal) -> ExternResult<()> {
        Self::err()
    }
    fn remote_signal(&self, _: RemoteSignal) -> ExternResult<()> {
        Self::err()
    }
    // Random
    fn random_bytes(&self, _: u32) -> ExternResult<Bytes> {
        Self::err()
    }
    // Time
    fn sys_time(&self, _: ()) -> ExternResult<Timestamp> {
        Self::err()
    }
    fn schedule(&self, _: String) -> ExternResult<()> {
        Self::err()
    }
    fn sleep(&self, _: std::time::Duration) -> ExternResult<()> {
        Self::err()
    }
    // XSalsa20Poly1305
    fn x_salsa20_poly1305_shared_secret_create_random(
        &self,
        _key_ref: Option<XSalsa20Poly1305KeyRef>,
    ) -> ExternResult<XSalsa20Poly1305KeyRef> {
        Self::err()
    }

    fn x_salsa20_poly1305_shared_secret_export(
        &self,
        _x_salsa20_poly1305_shared_secret_export: XSalsa20Poly1305SharedSecretExport,
    ) -> ExternResult<XSalsa20Poly1305EncryptedData> {
        Self::err()
    }

    fn x_salsa20_poly1305_shared_secret_ingest(
        &self,
        _x_salsa20_poly1305_shared_secret_ingest: XSalsa20Poly1305SharedSecretIngest,
    ) -> ExternResult<XSalsa20Poly1305KeyRef> {
        Self::err()
    }

    fn x_salsa20_poly1305_encrypt(
        &self,
        _x_salsa20_poly1305_encrypt: XSalsa20Poly1305Encrypt,
    ) -> ExternResult<XSalsa20Poly1305EncryptedData> {
        Self::err()
    }

    fn create_x25519_keypair(
        &self,
        _create_x25519_keypair_input: (),
    ) -> ExternResult<X25519PubKey> {
        Self::err()
    }

    fn x_25519_x_salsa20_poly1305_encrypt(
        &self,
        _x_25519_x_salsa20_poly1305_encrypt: X25519XSalsa20Poly1305Encrypt,
    ) -> ExternResult<XSalsa20Poly1305EncryptedData> {
        Self::err()
    }
}

/// The HDK implemented as externs provided by the host.
pub struct HostHdk;

#[cfg(all(not(feature = "mock"), target_arch = "wasm32"))]
use holochain_deterministic_integrity::hdi::HostHdi;

#[cfg(all(not(feature = "mock"), target_arch = "wasm32"))]
impl HdiT for HostHdk {
    fn verify_signature(&self, verify_signature: VerifySignature) -> ExternResult<bool> {
        HostHdi::new().verify_signature(verify_signature)
    }
    fn hash(&self, hash_input: HashInput) -> ExternResult<HashOutput> {
        HostHdi::new().hash(hash_input)
    }
    fn must_get_entry(&self, must_get_entry_input: MustGetEntryInput) -> ExternResult<EntryHashed> {
        HostHdi::new().must_get_entry(must_get_entry_input)
    }
    fn must_get_action(
        &self,
        must_get_action_input: MustGetActionInput,
    ) -> ExternResult<SignedActionHashed> {
        HostHdi::new().must_get_action(must_get_action_input)
    }
    fn must_get_valid_element(
        &self,
        must_get_valid_element_input: MustGetValidElementInput,
    ) -> ExternResult<Element> {
        HostHdi::new().must_get_valid_element(must_get_valid_element_input)
    }
    fn dna_info(&self, _: ()) -> ExternResult<DnaInfo> {
        HostHdi::new().dna_info(())
    }
    fn zome_info(&self, _: ()) -> ExternResult<ZomeInfo> {
        HostHdi::new().zome_info(())
    }
    fn trace(&self, m: TraceMsg) -> ExternResult<()> {
        HostHdi::new().trace(m)
    }
    fn x_salsa20_poly1305_decrypt(
        &self,
        x_salsa20_poly1305_decrypt: XSalsa20Poly1305Decrypt,
    ) -> ExternResult<Option<XSalsa20Poly1305Data>> {
        HostHdi::new().x_salsa20_poly1305_decrypt(x_salsa20_poly1305_decrypt)
    }
    fn x_25519_x_salsa20_poly1305_decrypt(
        &self,
        x_25519_x_salsa20_poly1305_decrypt: X25519XSalsa20Poly1305Decrypt,
    ) -> ExternResult<Option<XSalsa20Poly1305Data>> {
        HostHdi::new().x_25519_x_salsa20_poly1305_decrypt(x_25519_x_salsa20_poly1305_decrypt)
    }
}

/// The real hdk implements `host_call` for every hdk function.
/// This is deferring to the standard `holochain_wasmer_guest` crate functionality.
/// Every function works exactly the same way with the same basic signatures and patterns.
/// Elsewhere in the hdk are more high level wrappers around this basic trait.
#[cfg(all(not(feature = "mock"), target_arch = "wasm32"))]
impl HdkT for HostHdk {
    fn get_agent_activity(
        &self,
        get_agent_activity_input: GetAgentActivityInput,
    ) -> ExternResult<AgentActivity> {
        host_call::<GetAgentActivityInput, AgentActivity>(
            __get_agent_activity,
            get_agent_activity_input,
        )
    }
    fn query(&self, filter: ChainQueryFilter) -> ExternResult<Vec<Element>> {
        host_call::<ChainQueryFilter, Vec<Element>>(__query, filter)
    }
    fn sign(&self, sign: Sign) -> ExternResult<Signature> {
        host_call::<Sign, Signature>(__sign, sign)
    }
    fn sign_ephemeral(&self, sign_ephemeral: SignEphemeral) -> ExternResult<EphemeralSignatures> {
        host_call::<SignEphemeral, EphemeralSignatures>(__sign_ephemeral, sign_ephemeral)
    }
    fn create(&self, create_input: CreateInput) -> ExternResult<ActionHash> {
        host_call::<CreateInput, ActionHash>(__create, create_input)
    }
    fn update(&self, update_input: UpdateInput) -> ExternResult<ActionHash> {
        host_call::<UpdateInput, ActionHash>(__update, update_input)
    }
    fn delete(&self, hash: DeleteInput) -> ExternResult<ActionHash> {
        host_call::<DeleteInput, ActionHash>(__delete, hash)
    }
    fn get(&self, get_inputs: Vec<GetInput>) -> ExternResult<Vec<Option<Element>>> {
        host_call::<Vec<GetInput>, Vec<Option<Element>>>(__get, get_inputs)
    }
    fn get_details(&self, get_inputs: Vec<GetInput>) -> ExternResult<Vec<Option<Details>>> {
        host_call::<Vec<GetInput>, Vec<Option<Details>>>(__get_details, get_inputs)
    }
    // CounterSigning
    fn accept_countersigning_preflight_request(
        &self,
        preflight_request: PreflightRequest,
    ) -> ExternResult<PreflightRequestAcceptance> {
        host_call::<PreflightRequest, PreflightRequestAcceptance>(
            __accept_countersigning_preflight_request,
            preflight_request,
        )
    }
    fn agent_info(&self, _: ()) -> ExternResult<AgentInfo> {
        host_call::<(), AgentInfo>(__agent_info, ())
    }
    fn call_info(&self, _: ()) -> ExternResult<CallInfo> {
        host_call::<(), CallInfo>(__call_info, ())
    }
    fn create_link(&self, create_link_input: CreateLinkInput) -> ExternResult<ActionHash> {
        host_call::<CreateLinkInput, ActionHash>(__create_link, create_link_input)
    }
    fn delete_link(&self, delete_link_input: DeleteLinkInput) -> ExternResult<ActionHash> {
        host_call::<DeleteLinkInput, ActionHash>(__delete_link, delete_link_input)
    }
    fn get_links(&self, get_links_input: Vec<GetLinksInput>) -> ExternResult<Vec<Vec<Link>>> {
        host_call::<Vec<GetLinksInput>, Vec<Vec<Link>>>(__get_links, get_links_input)
    }
    fn get_link_details(
        &self,
        get_links_input: Vec<GetLinksInput>,
    ) -> ExternResult<Vec<LinkDetails>> {
        host_call::<Vec<GetLinksInput>, Vec<LinkDetails>>(__get_link_details, get_links_input)
    }
    fn call(&self, call: Vec<Call>) -> ExternResult<Vec<ZomeCallResponse>> {
        host_call::<Vec<Call>, Vec<ZomeCallResponse>>(__call, call)
    }
    fn emit_signal(&self, app_signal: AppSignal) -> ExternResult<()> {
        host_call::<AppSignal, ()>(__emit_signal, app_signal)
    }
    fn remote_signal(&self, remote_signal: RemoteSignal) -> ExternResult<()> {
        host_call::<RemoteSignal, ()>(__remote_signal, remote_signal)
    }
    fn random_bytes(&self, number_of_bytes: u32) -> ExternResult<Bytes> {
        host_call::<u32, Bytes>(__random_bytes, number_of_bytes)
    }
    fn sys_time(&self, _: ()) -> ExternResult<Timestamp> {
        host_call::<(), Timestamp>(__sys_time, ())
    }
    fn schedule(&self, scheduled_fn: String) -> ExternResult<()> {
        host_call::<String, ()>(__schedule, scheduled_fn)
    }
    fn sleep(&self, wake_after: std::time::Duration) -> ExternResult<()> {
        host_call::<std::time::Duration, ()>(__sleep, wake_after)
    }

    fn x_salsa20_poly1305_shared_secret_create_random(
        &self,
        key_ref: Option<XSalsa20Poly1305KeyRef>,
    ) -> ExternResult<XSalsa20Poly1305KeyRef> {
        host_call::<Option<XSalsa20Poly1305KeyRef>, XSalsa20Poly1305KeyRef>(
            __x_salsa20_poly1305_shared_secret_create_random,
            key_ref,
        )
    }

    fn x_salsa20_poly1305_shared_secret_export(
        &self,
        x_salsa20_poly1305_shared_secret_export: XSalsa20Poly1305SharedSecretExport,
    ) -> ExternResult<XSalsa20Poly1305EncryptedData> {
        host_call::<XSalsa20Poly1305SharedSecretExport, XSalsa20Poly1305EncryptedData>(
            __x_salsa20_poly1305_shared_secret_export,
            x_salsa20_poly1305_shared_secret_export,
        )
    }

    fn x_salsa20_poly1305_shared_secret_ingest(
        &self,
        x_salsa20_poly1305_shared_secret_ingest: XSalsa20Poly1305SharedSecretIngest,
    ) -> ExternResult<XSalsa20Poly1305KeyRef> {
        host_call::<XSalsa20Poly1305SharedSecretIngest, XSalsa20Poly1305KeyRef>(
            __x_salsa20_poly1305_shared_secret_ingest,
            x_salsa20_poly1305_shared_secret_ingest,
        )
    }

    fn x_salsa20_poly1305_encrypt(
        &self,
        x_salsa20_poly1305_encrypt: XSalsa20Poly1305Encrypt,
    ) -> ExternResult<XSalsa20Poly1305EncryptedData> {
        host_call::<XSalsa20Poly1305Encrypt, XSalsa20Poly1305EncryptedData>(
            __x_salsa20_poly1305_encrypt,
            x_salsa20_poly1305_encrypt,
        )
    }

    fn create_x25519_keypair(&self, _: ()) -> ExternResult<X25519PubKey> {
        host_call::<(), X25519PubKey>(__create_x25519_keypair, ())
    }

    fn x_25519_x_salsa20_poly1305_encrypt(
        &self,
        x_25519_x_salsa20_poly1305_encrypt: X25519XSalsa20Poly1305Encrypt,
    ) -> ExternResult<XSalsa20Poly1305EncryptedData> {
        host_call::<X25519XSalsa20Poly1305Encrypt, XSalsa20Poly1305EncryptedData>(
            __x_25519_x_salsa20_poly1305_encrypt,
            x_25519_x_salsa20_poly1305_encrypt,
        )
    }
}

/// At any time the global HDK can be set to a different hdk.
/// Generally this is only useful during rust unit testing.
/// When executing wasm without the `mock` feature, the host will be assumed.
pub fn set_hdk<H: 'static>(hdk: H)
where
    H: HdkT,
{
    let hdk = Rc::new(hdk);
    let hdk2 = hdk.clone();
    HDK.with(|h| {
        *h.borrow_mut() = hdk2;
    });
    holochain_deterministic_integrity::hdi::HDI.with(|h| {
        *h.borrow_mut() = hdk;
    });
}
