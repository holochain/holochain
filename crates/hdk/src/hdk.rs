use crate::prelude::*;

#[cfg(feature = "mock")]
use mockall::*;

pub const HDK_NOT_REGISTERED: &str = "HDK not registered";

/// This is a cell so it can be set many times.
/// Every test needs its own mock so each test needs to set it.
use core::cell::RefCell;

#[cfg(any(feature = "mock", not(target_arch = "wasm32")))]
thread_local!(pub static HDK: RefCell<Box<dyn HdkT>> = RefCell::new(Box::new(ErrHdk)));

#[cfg(all(not(feature = "mock"), target_arch = "wasm32"))]
thread_local!(pub static HDK: RefCell<Box<dyn HdkT>> = RefCell::new(Box::new(HostHdk)));

/// When mocking is enabled the mockall crate automatically builds a MockHdkT for us.
/// ```ignore
/// let mut mock = MockHdkT::new();
/// mock_hdk.expect_foo().times(1).etc().etc();
/// set_hdk(mock_hdk);
/// ```
#[cfg_attr(feature = "mock", automock)]
pub trait HdkT: Send + Sync {
    // Chain
    fn get_agent_activity(
        &self,
        get_agent_activity_input: GetAgentActivityInput,
    ) -> ExternResult<AgentActivity>;
    fn query(&self, filter: ChainQueryFilter) -> ExternResult<Vec<Element>>;
    // Ed25519
    fn sign(&self, sign: Sign) -> ExternResult<Signature>;
    fn sign_ephemeral(&self, sign_ephemeral: SignEphemeral) -> ExternResult<EphemeralSignatures>;
    fn verify_signature(&self, verify_signature: VerifySignature) -> ExternResult<bool>;
    // Entry
    fn create(&self, entry_with_def_id: EntryWithDefId) -> ExternResult<HeaderHash>;
    fn update(&self, update_input: UpdateInput) -> ExternResult<HeaderHash>;
    fn delete(&self, hash: HeaderHash) -> ExternResult<HeaderHash>;
    fn hash_entry(&self, entry: Entry) -> ExternResult<EntryHash>;
    fn get(&self, get_input: GetInput) -> ExternResult<Option<Element>>;
    fn get_details(&self, get_input: GetInput) -> ExternResult<Option<Details>>;
    fn must_get_entry(&self, must_get_entry_input: MustGetEntryInput) -> ExternResult<EntryHashed>;
    fn must_get_header(
        &self,
        must_get_header_input: MustGetHeaderInput,
    ) -> ExternResult<SignedHeaderHashed>;
    fn must_get_valid_element(
        &self,
        must_get_valid_element_input: MustGetValidElementInput,
    ) -> ExternResult<Element>;
    // Info
    fn agent_info(&self, agent_info_input: ()) -> ExternResult<AgentInfo>;
    fn app_info(&self, app_info_input: ()) -> ExternResult<AppInfo>;
    fn dna_info(&self, dna_info_input: ()) -> ExternResult<DnaInfo>;
    fn zome_info(&self, zome_info_input: ()) -> ExternResult<ZomeInfo>;
    fn call_info(&self, call_info_input: ()) -> ExternResult<CallInfo>;
    // Link
    fn create_link(&self, create_link_input: CreateLinkInput) -> ExternResult<HeaderHash>;
    fn delete_link(&self, add_link_header: HeaderHash) -> ExternResult<HeaderHash>;
    fn get_links(&self, get_links_input: GetLinksInput) -> ExternResult<Links>;
    fn get_link_details(&self, get_links_input: GetLinksInput) -> ExternResult<LinkDetails>;
    // P2P
    fn call(&self, call: Call) -> ExternResult<ZomeCallResponse>;
    fn call_remote(&self, call_remote: CallRemote) -> ExternResult<Vec<ZomeCallResponse>>;
    fn emit_signal(&self, app_signal: AppSignal) -> ExternResult<()>;
    fn remote_signal(&self, remote_signal: RemoteSignal) -> ExternResult<()>;
    // Random
    fn random_bytes(&self, number_of_bytes: u32) -> ExternResult<Bytes>;
    // Time
    fn sys_time(&self, sys_time_input: ()) -> ExternResult<core::time::Duration>;
    fn schedule(&self, execute_after: std::time::Duration) -> ExternResult<()>;
    fn sleep(&self, wake_after: std::time::Duration) -> ExternResult<()>;
    // Trace
    fn trace(&self, trace_msg: TraceMsg) -> ExternResult<()>;
    // XSalsa20Poly1305
    fn create_x25519_keypair(&self, create_x25519_keypair_input: ()) -> ExternResult<X25519PubKey>;
    fn x_salsa20_poly1305_decrypt(
        &self,
        x_salsa20_poly1305_decrypt: XSalsa20Poly1305Decrypt,
    ) -> ExternResult<Option<XSalsa20Poly1305Data>>;
    fn x_salsa20_poly1305_encrypt(
        &self,
        x_salsa20_poly1305_encrypt: XSalsa20Poly1305Encrypt,
    ) -> ExternResult<XSalsa20Poly1305EncryptedData>;
    fn x_25519_x_salsa20_poly1305_encrypt(
        &self,
        x_25519_x_salsa20_poly1305_encrypt: X25519XSalsa20Poly1305Encrypt,
    ) -> ExternResult<XSalsa20Poly1305EncryptedData>;
    fn x_25519_x_salsa20_poly1305_decrypt(
        &self,
        x_25519_x_salsa20_poly1305_decrypt: X25519XSalsa20Poly1305Decrypt,
    ) -> ExternResult<Option<XSalsa20Poly1305Data>>;
}

/// Used as a placeholder before any other Hdk is registered.
/// Generally only useful for testing but technically can be set any time.
pub struct ErrHdk;

impl ErrHdk {
    fn err<T>() -> ExternResult<T> {
        Err(WasmError::Guest(HDK_NOT_REGISTERED.to_string()))
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
    fn verify_signature(&self, _: VerifySignature) -> ExternResult<bool> {
        Self::err()
    }
    fn create(&self, _: EntryWithDefId) -> ExternResult<HeaderHash> {
        Self::err()
    }
    fn update(&self, _: UpdateInput) -> ExternResult<HeaderHash> {
        Self::err()
    }
    fn delete(&self, _: HeaderHash) -> ExternResult<HeaderHash> {
        Self::err()
    }
    fn hash_entry(&self, _: Entry) -> ExternResult<EntryHash> {
        Self::err()
    }
    fn get(&self, _: GetInput) -> ExternResult<Option<Element>> {
        Self::err()
    }
    fn get_details(&self, _: GetInput) -> ExternResult<Option<Details>> {
        Self::err()
    }
    fn must_get_entry(&self, _: MustGetEntryInput) -> ExternResult<EntryHashed> {
        Self::err()
    }
    fn must_get_header(&self, _: MustGetHeaderInput) -> ExternResult<SignedHeaderHashed> {
        Self::err()
    }
    fn must_get_valid_element(&self, _: MustGetValidElementInput) -> ExternResult<Element> {
        Self::err()
    }
    fn agent_info(&self, _: ()) -> ExternResult<AgentInfo> {
        Self::err()
    }
    fn app_info(&self, _: ()) -> ExternResult<AppInfo> {
        Self::err()
    }
    fn dna_info(&self, _: ()) -> ExternResult<DnaInfo> {
        Self::err()
    }
    fn zome_info(&self, _: ()) -> ExternResult<ZomeInfo> {
        Self::err()
    }
    fn call_info(&self, _: ()) -> ExternResult<CallInfo> {
        Self::err()
    }
    // Link
    fn create_link(&self, _: CreateLinkInput) -> ExternResult<HeaderHash> {
        Self::err()
    }
    fn delete_link(&self, _: HeaderHash) -> ExternResult<HeaderHash> {
        Self::err()
    }
    fn get_links(&self, _: GetLinksInput) -> ExternResult<Links> {
        Self::err()
    }
    fn get_link_details(&self, _: GetLinksInput) -> ExternResult<LinkDetails> {
        Self::err()
    }
    // P2P
    fn call(&self, _: Call) -> ExternResult<ZomeCallResponse> {
        Self::err()
    }
    fn call_remote(&self, _: CallRemote) -> ExternResult<Vec<ZomeCallResponse>> {
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
    fn sys_time(&self, _: ()) -> ExternResult<core::time::Duration> {
        Self::err()
    }
    fn schedule(&self, _: std::time::Duration) -> ExternResult<()> {
        Self::err()
    }
    fn sleep(&self, _: std::time::Duration) -> ExternResult<()> {
        Self::err()
    }
    // Trace
    fn trace(&self, _: TraceMsg) -> ExternResult<()> {
        Self::err()
    }
    // XSalsa20Poly1305
    fn create_x25519_keypair(&self, _: ()) -> ExternResult<X25519PubKey> {
        Self::err()
    }
    fn x_salsa20_poly1305_decrypt(
        &self,
        _: XSalsa20Poly1305Decrypt,
    ) -> ExternResult<Option<XSalsa20Poly1305Data>> {
        Self::err()
    }
    fn x_salsa20_poly1305_encrypt(
        &self,
        _: XSalsa20Poly1305Encrypt,
    ) -> ExternResult<XSalsa20Poly1305EncryptedData> {
        Self::err()
    }
    fn x_25519_x_salsa20_poly1305_encrypt(
        &self,
        _: X25519XSalsa20Poly1305Encrypt,
    ) -> ExternResult<XSalsa20Poly1305EncryptedData> {
        Self::err()
    }
    fn x_25519_x_salsa20_poly1305_decrypt(
        &self,
        _: X25519XSalsa20Poly1305Decrypt,
    ) -> ExternResult<Option<XSalsa20Poly1305Data>> {
        Self::err()
    }
}

/// The HDK implemented as externs provided by the host.
pub struct HostHdk;

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
    fn verify_signature(&self, verify_signature: VerifySignature) -> ExternResult<bool> {
        host_call::<VerifySignature, bool>(__verify_signature, verify_signature)
    }

    fn create(&self, entry_with_def_id: EntryWithDefId) -> ExternResult<HeaderHash> {
        host_call::<EntryWithDefId, HeaderHash>(__create, entry_with_def_id)
    }
    fn update(&self, update_input: UpdateInput) -> ExternResult<HeaderHash> {
        host_call::<UpdateInput, HeaderHash>(__update, update_input)
    }
    fn delete(&self, hash: HeaderHash) -> ExternResult<HeaderHash> {
        host_call::<HeaderHash, HeaderHash>(__delete, hash)
    }
    fn hash_entry(&self, entry: Entry) -> ExternResult<EntryHash> {
        host_call::<Entry, EntryHash>(__hash_entry, entry)
    }
    fn get(&self, get_input: GetInput) -> ExternResult<Option<Element>> {
        host_call::<GetInput, Option<Element>>(__get, get_input)
    }
    fn get_details(&self, get_input: GetInput) -> ExternResult<Option<Details>> {
        host_call::<GetInput, Option<Details>>(__get_details, get_input)
    }
    fn must_get_entry(&self, must_get_entry_input: MustGetEntryInput) -> ExternResult<EntryHashed> {
        host_call::<MustGetEntryInput, EntryHashed>(__must_get_entry, must_get_entry_input)
    }
    fn must_get_header(
        &self,
        must_get_header_input: MustGetHeaderInput,
    ) -> ExternResult<SignedHeaderHashed> {
        host_call::<MustGetHeaderInput, SignedHeaderHashed>(
            __must_get_header,
            must_get_header_input,
        )
    }
    fn must_get_valid_element(
        &self,
        must_get_valid_element_input: MustGetValidElementInput,
    ) -> ExternResult<Element> {
        host_call::<MustGetValidElementInput, Element>(
            __must_get_valid_element,
            must_get_valid_element_input,
        )
    }
    fn agent_info(&self, _: ()) -> ExternResult<AgentInfo> {
        host_call::<(), AgentInfo>(__agent_info, ())
    }
    fn app_info(&self, _: ()) -> ExternResult<AppInfo> {
        host_call::<(), AppInfo>(__app_info, ())
    }
    fn dna_info(&self, _: ()) -> ExternResult<DnaInfo> {
        host_call::<(), DnaInfo>(__dna_info, ())
    }
    fn zome_info(&self, _: ()) -> ExternResult<ZomeInfo> {
        host_call::<(), ZomeInfo>(__zome_info, ())
    }
    fn call_info(&self, _: ()) -> ExternResult<CallInfo> {
        host_call::<(), CallInfo>(__call_info, ())
    }

    fn create_link(&self, create_link_input: CreateLinkInput) -> ExternResult<HeaderHash> {
        host_call::<CreateLinkInput, HeaderHash>(__create_link, create_link_input)
    }
    fn delete_link(&self, add_link_header: HeaderHash) -> ExternResult<HeaderHash> {
        host_call::<HeaderHash, HeaderHash>(__delete_link, add_link_header)
    }
    fn get_links(&self, get_links_input: GetLinksInput) -> ExternResult<Links> {
        host_call::<GetLinksInput, Links>(__get_links, get_links_input)
    }
    fn get_link_details(&self, get_links_input: GetLinksInput) -> ExternResult<LinkDetails> {
        host_call::<GetLinksInput, LinkDetails>(__get_link_details, get_links_input)
    }

    fn call(&self, call: Call) -> ExternResult<ZomeCallResponse> {
        host_call::<Call, ZomeCallResponse>(__call, call)
    }
    fn call_remote(&self, call_remote: CallRemote) -> ExternResult<Vec<ZomeCallResponse>> {
        host_call::<CallRemote, Vec<ZomeCallResponse>>(__call_remote, call_remote)
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

    fn sys_time(&self, _: ()) -> ExternResult<core::time::Duration> {
        host_call::<(), core::time::Duration>(__sys_time, ())
    }
    fn schedule(&self, execute_after: std::time::Duration) -> ExternResult<()> {
        host_call::<std::time::Duration, ()>(__schedule, execute_after)
    }
    fn sleep(&self, wake_after: std::time::Duration) -> ExternResult<()> {
        host_call::<std::time::Duration, ()>(__sleep, wake_after)
    }

    fn trace(&self, trace_msg: TraceMsg) -> ExternResult<()> {
        host_call::<TraceMsg, ()>(__trace, trace_msg)
    }

    fn create_x25519_keypair(&self, _: ()) -> ExternResult<X25519PubKey> {
        host_call::<(), X25519PubKey>(__create_x25519_keypair, ())
    }
    fn x_salsa20_poly1305_decrypt(
        &self,
        x_salsa20_poly1305_decrypt: XSalsa20Poly1305Decrypt,
    ) -> ExternResult<Option<XSalsa20Poly1305Data>> {
        host_call::<XSalsa20Poly1305Decrypt, Option<XSalsa20Poly1305Data>>(
            __x_salsa20_poly1305_decrypt,
            x_salsa20_poly1305_decrypt,
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
    fn x_25519_x_salsa20_poly1305_encrypt(
        &self,
        x_25519_x_salsa20_poly1305_encrypt: X25519XSalsa20Poly1305Encrypt,
    ) -> ExternResult<XSalsa20Poly1305EncryptedData> {
        host_call::<X25519XSalsa20Poly1305Encrypt, XSalsa20Poly1305EncryptedData>(
            __x_25519_x_salsa20_poly1305_encrypt,
            x_25519_x_salsa20_poly1305_encrypt,
        )
    }
    fn x_25519_x_salsa20_poly1305_decrypt(
        &self,
        x_25519_x_salsa20_poly1305_decrypt: X25519XSalsa20Poly1305Decrypt,
    ) -> ExternResult<Option<XSalsa20Poly1305Data>> {
        host_call::<X25519XSalsa20Poly1305Decrypt, Option<XSalsa20Poly1305Data>>(
            __x_25519_x_salsa20_poly1305_decrypt,
            x_25519_x_salsa20_poly1305_decrypt,
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
    HDK.with(|h| {
        *h.borrow_mut() = Box::new(hdk);
    });
}
