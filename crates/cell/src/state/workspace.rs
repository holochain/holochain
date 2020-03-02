use sx_state::{
    buffer::{KvBuffer, KvvBuffer, StoreBuffer},
    error::WorkspaceResult, db::DbManager,
    Writer, Reader,
};
use super::chain_cas::ChainCasBuffer;

pub trait Workspace<'txn>: Sized {
    fn finalize(self, writer: Writer) -> WorkspaceResult<()>;
}

pub struct InvokeZomeWorkspace<'env> {
    cas: ChainCasBuffer<'env>,
    // meta: KvvBuffer<'env, String, String>,
}

// impl<'env> Workspace<'env> for InvokeZomeWorkspace<'env> {
//     fn finalize(self, mut writer: Writer) -> WorkspaceResult<()> {
//         self.cas.finalize(&mut writer)?;
//         // self.meta.finalize(&mut writer)?;
//         writer.commit()?;
//         Ok(())
//     }
// }

impl<'env> InvokeZomeWorkspace<'env> {
    pub fn new(reader: &'env Reader<'env>, dbm: &'env DbManager<'env>) -> WorkspaceResult<Self> {
        Ok(Self {
            cas: ChainCasBuffer::primary(reader, dbm)?,
            // meta: KvvBuffer::new(reader, dbm)?,
        })
    }

    pub fn cas(&mut self) -> &mut ChainCasBuffer<'env> {
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
    use sx_state::{db::DbManager, env::create_lmdb_env};
    use sx_types::prelude::*;
    use tempdir::TempDir;

    #[test]
    fn workspace_sanity_check() {
        let tmpdir = TempDir::new("skunkworx").unwrap();
        let created_arc = create_lmdb_env(tmpdir.path());
        let env = created_arc.read().unwrap();
        let dbm = DbManager::new(&env).unwrap();
        let address = Address::from("hi".to_owned());
        let reader = env.read().unwrap();

        let mut workspace = InvokeZomeWorkspace::new(&reader, &dbm).unwrap();
        let cas = workspace.cas();
        assert_eq!(cas.get_entry(&address).unwrap(), None);

        panic!("Rewrite this test using a fake TestWorkspace")

        // TODO: rewrite with real entries and headers

        // cas.put("hi".to_owned(), "there".to_owned());
        // assert_eq!(cas.get(&"hi".to_owned()).unwrap(), Some("there".to_owned()));
        // workspace.finalize(env.write().unwrap()).unwrap();

        // // Ensure that the data was persisted
        // let mut workspace = InvokeZomeWorkspace::new(&env).unwrap();
        // assert_eq!(
        //     workspace.cas().get(&"hi".to_owned()).unwrap(),
        //     Some("there".to_owned())
        // );
    }
}
