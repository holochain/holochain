

use sx_types::Address;

trait NetRequester {
    fn fetch_entry(address: Address);
}
