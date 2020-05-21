extern crate wee_alloc;

use holochain_wasmer_guest::*;
use holochain_zome_types::*;
use holochain_zome_types::validate::ValidationPackageCallbackResult;
use holochain_zome_types::validate::ValidationPackage;

// Use `wee_alloc` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// define the host functions we require in order to pull/push data across the host/guest boundary
memory_externs!();

host_externs!(
    __globals,
    __call,
    __capability,
    __commit_entry,
    __decrypt,
    __encrypt,
    __show_env,
    __property,
    __query,
    __remove_link,
    __send,
    __sign,
    __schedule,
    __update_entry,
    __emit_signal,
    __remove_entry,
    __link_entries,
    __keystore,
    __get_links,
    __get_entry,
    __entry_type_properties,
    __entry_address,
    __sys_time,
    __debug
);


#[no_mangle]
pub extern "C" fn validation_package(_: RemotePtr) -> RemotePtr {
    ret!(GuestOutput::new(try_result!(ValidationPackageCallbackResult::Success(ValidationPackage).try_into(), "failed to serialize validation package return value")));
}
