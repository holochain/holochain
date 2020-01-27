

use crate::shims::Address;

trait NetRequester {
    fn fetch_entry(address: Address);
}
