use holochain_types::dna::zome::inline_zome::InlineZome;
use holochain_zome_types::zome::{FunctionName, ZomeName};
use std::collections::HashMap;

mod ribosome;

use super::error::RibosomeResult;

#[derive(Debug)]
pub struct InlineDna {
    zomes: HashMap<ZomeName, InlineZome>,
}

impl InlineDna {
    pub fn new(zomes: HashMap<ZomeName, InlineZome>) -> Self {
        Self { zomes }
    }
}
