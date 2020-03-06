use super::chain_cas::ChainCasBuffer;
use sx_state::{
    buffer::{KvBuffer, KvvBuffer, StoreBuffer},
    db::DbManager,
    error::WorkspaceResult,
    Reader, Writer,
};

mod genesis;
pub use genesis::GenesisWorkspace;

pub trait Workspace<'txn>: Sized {
    fn commit_txn(self, writer: Writer) -> WorkspaceResult<()>;
}

pub struct InvokeZomeWorkspace<'env> {
    cas: ChainCasBuffer<'env>,
    // meta: KvvBuffer<'env, String, String>,
}

// impl<'env> Workspace<'env> for InvokeZomeWorkspace<'env> {
//     fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
//         self.cas.flush_to_txn(&mut writer)?;
//         // self.meta.flush_to_txn(&mut writer)?;
//         writer.commit()?;
//         Ok(())
//     }
// }

impl<'env> InvokeZomeWorkspace<'env> {
    pub fn new(reader: &'env Reader<'env>, dbs: &'env DbManager<'env>) -> WorkspaceResult<Self> {
        Ok(Self {
            cas: ChainCasBuffer::primary(reader, dbs)?,
            // meta: KvvBuffer::new(reader, dbs)?,
        })
    }

    pub fn cas(&mut self) -> &mut ChainCasBuffer<'env> {
        &mut self.cas
    }
}

pub struct AppValidationWorkspace;

impl<'env> Workspace<'env> for AppValidationWorkspace {
    fn commit_txn(self, _writer: Writer) -> WorkspaceResult<()> {
        unimplemented!()
    }
}

#[cfg(test)]
pub mod tests {

    use super::{InvokeZomeWorkspace, Workspace};
    use sx_state::{
        buffer::{KvBuffer, StoreBuffer},
        db::{DbManager, CHAIN_ENTRIES, CHAIN_HEADERS},
        env::{ReadManager, WriteManager},
        error::WorkspaceResult,
        test_utils::test_env,
        Reader, SingleStore, Writer,
    };
    use sx_types::prelude::*;
    use tempdir::TempDir;

    struct TestWorkspace<'env> {
        one: KvBuffer<'env, Address, u32>,
        two: KvBuffer<'env, Address, bool>,
    }

    impl<'env> TestWorkspace<'env> {
        pub fn new(
            reader: &'env Reader<'env>,
            dbs: &'env DbManager<'env>,
        ) -> WorkspaceResult<Self> {
            Ok(Self {
                one: KvBuffer::new(reader, *dbs.get(&*CHAIN_ENTRIES)?)?,
                two: KvBuffer::new(reader, *dbs.get(&*CHAIN_HEADERS)?)?,
            })
        }
    }

    impl<'env> Workspace<'env> for TestWorkspace<'env> {
        fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
            self.one.flush_to_txn(&mut writer)?;
            self.two.flush_to_txn(&mut writer)?;
            writer.commit()?;
            Ok(())
        }
    }

    #[test]
    fn workspace_sanity_check() {
        let arc = test_env();
        let env = arc.env();
        let dbs = arc.dbs().unwrap();
        let addr1 = Address::from("hi".to_owned());
        let addr2 = Address::from("hi".to_owned());
        {
            let reader = env.reader().unwrap();
            let mut workspace = TestWorkspace::new(&reader, &dbs).unwrap();
            assert_eq!(workspace.one.get(&addr1).unwrap(), None);

            workspace.one.put(addr1.clone(), 1);
            workspace.two.put(addr2.clone(), true);
            assert_eq!(workspace.one.get(&addr1).unwrap(), Some(1));
            assert_eq!(workspace.two.get(&addr2).unwrap(), Some(true));
            workspace.commit_txn(env.writer().unwrap()).unwrap();
        }

        // Ensure that the data was persisted
        {
            let reader = env.reader().unwrap();
            let workspace = TestWorkspace::new(&reader, &dbs).unwrap();
            assert_eq!(workspace.one.get(&addr1).unwrap(), Some(1));
        }
    }
}
