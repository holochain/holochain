use holochain_types::dna::zome::inline_zome::InlineZome;
use holochain_zome_types::zome::{FunctionName, ZomeName};
use std::collections::HashMap;

mod ribosome;

use super::error::RibosomeResult;

#[derive(Debug)]
pub struct InlineDna<'f> {
    zomes: HashMap<ZomeName, InlineZome<'f>>,
}

impl<'f> InlineDna<'f> {
    pub fn new(zomes: HashMap<ZomeName, InlineZome<'f>>) -> Self {
        Self { zomes }
    }
}
