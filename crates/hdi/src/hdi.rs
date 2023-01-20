use crate::prelude::*;

pub const HDI_NOT_REGISTERED: &str = "HDI not registered";

/// This is a cell so it can be set many times.
/// Every test needs its own mock so each test needs to set it.
use core::cell::RefCell;
use std::rc::Rc;

#[cfg(any(feature = "mock", not(target_arch = "wasm32")))]
thread_local!(pub static HDI: RefCell<Rc<dyn HdiT>> = RefCell::new(Rc::new(ErrHdi)));

#[cfg(all(not(feature = "mock"), target_arch = "wasm32"))]
thread_local!(pub static HDI: RefCell<Rc<dyn HdiT>> = RefCell::new(Rc::new(HostHdi)));

/// When mocking is enabled the mockall crate automatically builds a MockHdiT for us.
/// ```ignore
/// let mut mock_hdi = MockHdiT::new();
/// mock_hdi.expect_foo().times(1).etc().etc();
/// set_hdi(mock_hdi);
/// ```
pub trait HdiT: Send + Sync {
    // Ed25519
    fn verify_signature(&self, verify_signature: VerifySignature) -> ExternResult<bool>;
    fn hash(&self, hash_input: HashInput) -> ExternResult<HashOutput>;
    fn must_get_entry(&self, must_get_entry_input: MustGetEntryInput) -> ExternResult<EntryHashed>;
    fn must_get_action(
        &self,
        must_get_action_input: MustGetActionInput,
    ) -> ExternResult<SignedActionHashed>;
    fn must_get_valid_record(
        &self,
        must_get_valid_record_input: MustGetValidRecordInput,
    ) -> ExternResult<Record>;
    fn must_get_agent_activity(
        &self,
        must_get_agent_activity_input: MustGetAgentActivityInput,
    ) -> ExternResult<Vec<RegisterAgentActivity>>;
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

/// Used as a placeholder before any other Hdi is registered.
/// Generally only useful for testing but technically can be set any time.
pub struct ErrHdi;

impl ErrHdi {
    fn err<T>() -> ExternResult<T> {
        Err(wasm_error!(WasmErrorInner::Guest(
            HDI_NOT_REGISTERED.to_string()
        )))
    }
}

/// Every call is an error for the ErrHdi.
impl HdiT for ErrHdi {
    fn verify_signature(&self, _: VerifySignature) -> ExternResult<bool> {
        Self::err()
    }
    fn hash(&self, _: HashInput) -> ExternResult<HashOutput> {
        Self::err()
    }
    fn must_get_entry(&self, _: MustGetEntryInput) -> ExternResult<EntryHashed> {
        Self::err()
    }
    fn must_get_action(&self, _: MustGetActionInput) -> ExternResult<SignedActionHashed> {
        Self::err()
    }
    fn must_get_valid_record(&self, _: MustGetValidRecordInput) -> ExternResult<Record> {
        Self::err()
    }
    fn must_get_agent_activity(
        &self,
        _: MustGetAgentActivityInput,
    ) -> ExternResult<Vec<RegisterAgentActivity>> {
        Self::err()
    }
    fn dna_info(&self, _: ()) -> ExternResult<DnaInfo> {
        Self::err()
    }
    fn zome_info(&self, _: ()) -> ExternResult<ZomeInfo> {
        Self::err()
    }
    // Trace
    fn trace(&self, _: TraceMsg) -> ExternResult<()> {
        Self::err()
    }
    fn x_salsa20_poly1305_decrypt(
        &self,
        _: XSalsa20Poly1305Decrypt,
    ) -> ExternResult<Option<XSalsa20Poly1305Data>> {
        Self::err()
    }
    fn x_25519_x_salsa20_poly1305_decrypt(
        &self,
        _: X25519XSalsa20Poly1305Decrypt,
    ) -> ExternResult<Option<XSalsa20Poly1305Data>> {
        Self::err()
    }
}

/// The HDI implemented as externs provided by the host.
pub struct HostHdi;

impl HostHdi {
    pub const fn new() -> Self {
        Self {}
    }
}

/// The real hdi implements `host_call` for every hdi function.
/// This is deferring to the standard `holochain_wasmer_guest` crate functionality.
/// Every function works exactly the same way with the same basic signatures and patterns.
/// Elsewhere in the hdi are more high level wrappers around this basic trait.
#[cfg(all(not(feature = "mock"), target_arch = "wasm32"))]
impl HdiT for HostHdi {
    fn verify_signature(&self, verify_signature: VerifySignature) -> ExternResult<bool> {
        host_call::<VerifySignature, bool>(__hc__verify_signature_1, verify_signature)
    }
    fn hash(&self, hash_input: HashInput) -> ExternResult<HashOutput> {
        host_call::<HashInput, HashOutput>(__hc__hash_1, hash_input)
    }
    fn must_get_entry(&self, must_get_entry_input: MustGetEntryInput) -> ExternResult<EntryHashed> {
        host_call::<MustGetEntryInput, EntryHashed>(__hc__must_get_entry_1, must_get_entry_input)
    }
    fn must_get_action(
        &self,
        must_get_action_input: MustGetActionInput,
    ) -> ExternResult<SignedActionHashed> {
        host_call::<MustGetActionInput, SignedActionHashed>(
            __hc__must_get_action_1,
            must_get_action_input,
        )
    }
    fn must_get_valid_record(
        &self,
        must_get_valid_record_input: MustGetValidRecordInput,
    ) -> ExternResult<Record> {
        host_call::<MustGetValidRecordInput, Record>(
            __hc__must_get_valid_record_1,
            must_get_valid_record_input,
        )
    }
    fn must_get_agent_activity(
        &self,
        must_get_agent_activity_input: MustGetAgentActivityInput,
    ) -> ExternResult<Vec<RegisterAgentActivity>> {
        host_call::<MustGetAgentActivityInput, Vec<RegisterAgentActivity>>(
            __hc__must_get_agent_activity_1,
            must_get_agent_activity_input,
        )
    }
    fn dna_info(&self, _: ()) -> ExternResult<DnaInfo> {
        host_call::<(), DnaInfo>(__hc__dna_info_1, ())
    }
    fn zome_info(&self, _: ()) -> ExternResult<ZomeInfo> {
        host_call::<(), ZomeInfo>(__hc__zome_info_1, ())
    }
    fn trace(&self, trace_msg: TraceMsg) -> ExternResult<()> {
        if cfg!(feature = "trace") {
            host_call::<TraceMsg, ()>(__hc__trace_1, trace_msg)
        } else {
            Err(wasm_error!(WasmErrorInner::Guest(
                "`trace()` can only be used when the \"trace\" cargo feature is set (it is off by default).".to_string(),
            )))
        }
    }
    fn x_salsa20_poly1305_decrypt(
        &self,
        x_salsa20_poly1305_decrypt: XSalsa20Poly1305Decrypt,
    ) -> ExternResult<Option<XSalsa20Poly1305Data>> {
        host_call::<XSalsa20Poly1305Decrypt, Option<XSalsa20Poly1305Data>>(
            __hc__x_salsa20_poly1305_decrypt_1,
            x_salsa20_poly1305_decrypt,
        )
    }
    fn x_25519_x_salsa20_poly1305_decrypt(
        &self,
        x_25519_x_salsa20_poly1305_decrypt: X25519XSalsa20Poly1305Decrypt,
    ) -> ExternResult<Option<XSalsa20Poly1305Data>> {
        host_call::<X25519XSalsa20Poly1305Decrypt, Option<XSalsa20Poly1305Data>>(
            __hc__x_25519_x_salsa20_poly1305_decrypt_1,
            x_25519_x_salsa20_poly1305_decrypt,
        )
    }
}

/// At any time the global HDI can be set to a different HDI.
/// Generally this is only useful during rust unit testing.
/// When executing wasm without the `mock` feature, the host will be assumed.
pub fn set_hdi<H: 'static>(hdi: H) -> Rc<dyn HdiT>
where
    H: HdiT,
{
    HDI.with(|h| std::mem::replace(&mut *h.borrow_mut(), Rc::new(hdi)))
}
