use core::ops::Deref;
use super::{chain_cas::{HeaderCas, ChainCasBuffer}, chain_sequence::ChainSequenceBuffer};
use sx_state::{error::WorkspaceResult, RkvEnv, buffer::StoreBuffer, Writer};
use sx_types::{chain_header::ChainHeader, entry::Entry, prelude::Address};

pub struct SourceChainBuffer<'env> {
    cas: ChainCasBuffer<'env>,
    sequence: ChainSequenceBuffer<'env>,
}

impl<'env> SourceChainBuffer<'env> {
    pub fn create(env: &'env RkvEnv) -> WorkspaceResult<Self> {
        Ok(Self {
            cas: ChainCasBuffer::create(env, "sourcechain")?,
            sequence: ChainSequenceBuffer::create(env)?,
        })
    }

    pub fn chain_head(&self) -> WorkspaceResult<Option<Address>> {
        self.sequence.chain_head()
    }

    pub fn get_entry(&self, k: &Address) -> WorkspaceResult<Option<Entry>> {
        self.cas.get_entry(k)
    }

    pub fn get_header(&self, k: &Address) -> WorkspaceResult<Option<ChainHeader>> {
        self.cas.get_header(k)
    }

    pub fn cas(&self) -> &ChainCasBuffer {
        &self.cas
    }

    pub fn put(&mut self, pair: (ChainHeader, Entry)) -> () {
        let (header, entry) = pair;
        self.cas.put(pair);
    }

    pub fn headers(&self) -> &HeaderCas<'env> {
        &self.cas.headers()
    }
}


impl<'env> StoreBuffer<'env> for SourceChainBuffer<'env>
{
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        self.cas.finalize(writer)?;
        self.sequence.finalize(writer)?;
        Ok(())
    }
}
