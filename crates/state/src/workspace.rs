use crate::{
    error::WorkspaceResult,
    store::{kv::KvBuffer, kvv::KvvBuffer, TransactionalStore},
};
use rkv::{Rkv, Writer};

pub trait Workspace<'txn>: Sized {
    fn finalize(self, writer: Writer) -> WorkspaceResult<()>;
}

pub struct InvokeZomeWorkspace<'env> {
    cas: KvBuffer<'env, String, String>,
    meta: KvvBuffer<'env, String, String>,
}

impl<'env> Workspace<'env> for InvokeZomeWorkspace<'env> {
    fn finalize(self, mut writer: Writer) -> WorkspaceResult<()> {
        self.cas.finalize(&mut writer)?;
        // self.meta.finalize(&mut writer)?;
        writer.commit()?;
        Ok(())
    }
}

impl<'env> InvokeZomeWorkspace<'env> {
    pub fn new(env: &'env Rkv) -> WorkspaceResult<Self> {
        Ok(Self {
            // TODO: careful with this create()
            cas: KvBuffer::create(env, "cas")?,
            meta: KvvBuffer::create(env, "meta")?,
        })
    }

    pub fn cas(&mut self) -> &mut KvBuffer<'env, String, String> {
        &mut self.cas
    }
}

pub struct AppValidationWorkspace;

impl<'env> Workspace<'env> for AppValidationWorkspace {
    fn finalize(self, _writer: Writer) -> WorkspaceResult<()> {
        unimplemented!()
    }
}

#[cfg(test)]
pub mod tests {

    use super::{InvokeZomeWorkspace, Workspace};
    use crate::env::create_lmdb_env;
    use tempdir::TempDir;

    #[test]
    fn workspace_sanity_check() {
        let tmpdir = TempDir::new("skunkworx").unwrap();
        let created_arc = create_lmdb_env(tmpdir.path());
        let env = created_arc.read().unwrap();

        let mut workspace = InvokeZomeWorkspace::new(&env).unwrap();
        let cas = workspace.cas();
        assert_eq!(cas.get(&"hi".to_owned()).unwrap(), None);
        cas.put("hi".to_owned(), "there".to_owned());
        assert_eq!(cas.get(&"hi".to_owned()).unwrap(), Some("there".to_owned()));
        workspace.finalize(env.write().unwrap()).unwrap();

        // Ensure that the data was persisted
        let mut workspace = InvokeZomeWorkspace::new(&env).unwrap();
        assert_eq!(
            workspace.cas().get(&"hi".to_owned()).unwrap(),
            Some("there".to_owned())
        );
    }
}
