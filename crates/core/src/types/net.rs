

use sx_types::shims::Address;

trait NetRequester {
    fn fetch_entry(address: Address);
}
