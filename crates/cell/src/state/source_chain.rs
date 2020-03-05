use super::{
    chain_cas::{ChainCasBuffer, HeaderCas},
    chain_sequence::ChainSequenceBuffer,
};
use core::ops::Deref;
use sx_state::{
    buffer::StoreBuffer,
    db::{self, DbManager},
    env::ReadManager,
    error::WorkspaceResult,
    Readable, Reader, Writer,
};
use sx_types::{
    chain_header::ChainHeader,
    entry::Entry,
    prelude::Address,
    signature::{Provenance, Signature},
};

pub struct SourceChainBuffer<'env, R: Readable, RM: ReadManager> {
    cas: ChainCasBuffer<'env, R>,
    sequence: ChainSequenceBuffer<'env, R>,
    rm: RM,
}

impl<'env, R: Readable, RM: ReadManager> SourceChainBuffer<'env, R, RM> {
    pub fn new(reader: &'env R, dbs: &'env DbManager, rm: RM) -> WorkspaceResult<Self> {
        Ok(Self {
            cas: ChainCasBuffer::primary(reader, dbs)?,
            sequence: ChainSequenceBuffer::new(reader, dbs)?,
            rm,
        })
    }

    pub fn chain_head(&self) -> Option<&Address> {
        self.sequence.chain_head()
    }

    pub fn get_entry(&self, k: &Address) -> WorkspaceResult<Option<Entry>> {
        self.cas.get_entry(k)
    }

    pub fn get_header(&self, k: &Address) -> WorkspaceResult<Option<ChainHeader>> {
        self.cas.get_header(k)
    }

    pub fn cas(&self) -> &ChainCasBuffer<R> {
        &self.cas
    }

    pub fn put_entry(&mut self, entry: Entry) -> () {
        let header = self.header_for_entry(&entry);
        self.cas.put((header, entry));
    }

    pub fn headers(&self) -> &HeaderCas<'env, R> {
        &self.cas.headers()
    }

    pub fn try_commit(&self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        unimplemented!()
    }

    fn header_for_entry(&self, entry: &Entry) -> ChainHeader {
        unimplemented!()
        // let provenances = &[Provenance::new(
        //     self.snapshot.agent_id().unwrap().address(),
        //     Signature::fake(),
        // )];
        // let timestamp = chrono::Utc::now().timestamp().into();
        // let header = ChainHeader::new(
        //     entry.entry_type(),
        //     entry.address(),
        //     provenances,
        //     Some(self.new_head.clone()),
        //     None,
        //     None,
        //     timestamp,
        // );
        // Ok(header)
    }
}

impl<'env, R: Readable, RM: ReadManager> StoreBuffer<'env> for SourceChainBuffer<'env, R, RM> {
    fn finalize(self, writer: &'env mut Writer) -> WorkspaceResult<()> {
        self.cas.finalize(writer)?;
        self.sequence.finalize(writer)?;
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {

    use super::{SourceChainBuffer, StoreBuffer};
    use sx_state::{
        env::{create_lmdb_env, DbManager, ReadManager, WriteManager},
        error::WorkspaceResult,
        test_utils::test_env,
        Reader,
    };
    use tempdir::TempDir;

    #[test]
    fn asdf() -> WorkspaceResult<()> {
        let arc = test_env();
        let env = arc.env();
        let dbs = arc.dbs()?;
        arc.env().with_reader(|reader| {
            let source_chain = SourceChainBuffer::new(&reader, &dbs, env)?;
            Ok(())
        })?;
        Ok(())
    }
}
