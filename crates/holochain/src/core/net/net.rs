use sx_types::prelude::*;

pub trait NetRequester {
    fn fetch_entry(address: Address);
}