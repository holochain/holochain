use crate::conductor::api::CellConductorApiT;
use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::InvocationAuth;
use crate::core::ribosome::ZomesToInvoke;
use derive_more::Constructor;
use holochain_keystore::MetaLairClient;
use holochain_p2p::HolochainP2pDna;
use holochain_p2p::HolochainP2pDnaT;
use holochain_serialized_bytes::prelude::*;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_types::prelude::*;
use itertools::Itertools;

pub const POST_COMMIT_CHANNEL_BOUND: usize = 100;
pub const POST_COMMIT_CONCURRENT_LIMIT: usize = 5;

#[derive(Clone)]
pub struct PostCommitInvocation {
    zome: Zome,
    headers: Vec<SignedHeaderHashed>,
}

impl PostCommitInvocation {
    pub fn new(zome: Zome, headers: Vec<SignedHeaderHashed>) -> Self {
        Self { zome, headers }
    }
}

#[derive(Clone, Constructor)]
pub struct PostCommitHostAccess {
    pub workspace: HostFnWorkspace,
    pub keystore: MetaLairClient,
    pub network: HolochainP2pDna,
}

impl From<PostCommitHostAccess> for HostContext {
    fn from(post_commit_host_access: PostCommitHostAccess) -> Self {
        Self::PostCommit(post_commit_host_access)
    }
}

impl From<&PostCommitHostAccess> for HostFnAccess {
    fn from(_: &PostCommitHostAccess) -> Self {
        let mut access = Self::all();
        // Post commit happens after all workspace writes are complete.
        // Writing more to the workspace becomes circular.
        // If you need to trigger some more writes, try a `call_remote` back
        // into the current cell.
        access.write_workspace = Permission::Deny;
        access
    }
}

impl Invocation for PostCommitInvocation {
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::One(self.zome.to_owned())
    }
    fn fn_components(&self) -> FnComponents {
        vec!["post_commit".into()].into()
    }
    fn host_input(self) -> Result<ExternIO, SerializedBytesError> {
        ExternIO::encode(self.headers)
    }
    fn auth(&self) -> InvocationAuth {
        InvocationAuth::LocalCallback
    }
}

impl TryFrom<PostCommitInvocation> for ExternIO {
    type Error = SerializedBytesError;
    fn try_from(post_commit_invocation: PostCommitInvocation) -> Result<Self, Self::Error> {
        ExternIO::encode(&post_commit_invocation.headers)
    }
}

pub async fn send_post_commit<C>(
    conductor_api: C,
    workspace: HostFnWorkspace,
    network: HolochainP2pDna,
    keystore: MetaLairClient,
    zomed_headers: Vec<(Option<Zome>, SignedHeaderHashed)>,
) -> Result<(), tokio::sync::mpsc::error::SendError<()>>
where
    C: CellConductorApiT,
{
    let groups = zomed_headers
        .iter()
        .group_by(|(zome, _shh)| zome.clone())
        .into_iter()
        .map(|(maybe_zome, group)| {
            (
                maybe_zome,
                group.map(|(_maybe_zome, shh)| shh.clone()).collect(),
            )
        })
        .collect::<Vec<(Option<Zome>, Vec<SignedHeaderHashed>)>>();

    for (maybe_zome, headers) in groups {
        if let Some(zome) = maybe_zome {
            let zome = zome.clone();
            conductor_api
                .post_commit_permit()
                .await?
                .send(PostCommitArgs {
                    host_access: PostCommitHostAccess {
                        workspace: workspace.clone(),
                        keystore: keystore.clone(),
                        network: network.clone(),
                    },
                    invocation: PostCommitInvocation::new(zome, headers),
                    cell_id: CellId::new(
                        network.dna_hash().clone(),
                        conductor_api.cell_id().agent_pubkey().clone(),
                    ),
                });
        }
    }
    Ok(())
}

#[derive(Clone)]
pub struct PostCommitArgs {
    pub host_access: PostCommitHostAccess,
    pub invocation: PostCommitInvocation,
    pub cell_id: CellId,
}

#[cfg(test)]
mod test {
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::fixt::PostCommitHostAccessFixturator;
    use crate::fixt::PostCommitInvocationFixturator;
    use holochain_types::prelude::*;
    use holochain_zome_types::ExternIO;

    #[tokio::test(flavor = "multi_thread")]
    async fn post_commit_invocation_access() {
        let post_commit_host_access = PostCommitHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let mut expected = HostFnAccess::all();
        expected.write_workspace = Permission::Deny;
        assert_eq!(HostFnAccess::from(&post_commit_host_access), expected);
    }

    #[test]
    fn post_commit_invocation_zomes() {
        let post_commit_invocation = PostCommitInvocationFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let zome = post_commit_invocation.zome.clone();
        assert_eq!(ZomesToInvoke::One(zome), post_commit_invocation.zomes(),);
    }

    #[test]
    fn post_commit_invocation_fn_components() {
        let post_commit_invocation = PostCommitInvocationFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();

        let mut expected = vec!["post_commit"];
        for fn_component in post_commit_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap());
        }
    }

    #[test]
    fn post_commit_invocation_host_input() {
        let post_commit_invocation = PostCommitInvocationFixturator::new(::fixt::Empty)
            .next()
            .unwrap();

        let host_input = post_commit_invocation.clone().host_input().unwrap();

        assert_eq!(
            host_input,
            ExternIO::encode(HeaderHashVecFixturator::new(::fixt::Empty).next().unwrap()).unwrap(),
        );
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {
    use crate::conductor::ConductorBuilder;
    use crate::core::ribosome::RibosomeT;
    use crate::fixt::curve::Zomes;
    use crate::fixt::PostCommitHostAccessFixturator;
    use crate::fixt::PostCommitInvocationFixturator;
    use crate::fixt::RealRibosomeFixturator;
    use crate::sweettest::SweetConductor;
    use crate::sweettest::SweetDnaFile;
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holochain_types::prelude::MockDnaStore;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_post_commit_unimplemented() {
        let host_access = PostCommitHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let mut post_commit_invocation = PostCommitInvocationFixturator::new(::fixt::Empty)
            .next()
            .unwrap();
        post_commit_invocation.zome = TestWasm::Foo.into();

        let result = ribosome
            .run_post_commit(host_access, post_commit_invocation)
            .unwrap();
        assert_eq!(result, ());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_post_commit_implemented_success() {
        let host_access = PostCommitHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::PostCommitSuccess]))
            .next()
            .unwrap();
        let mut post_commit_invocation = PostCommitInvocationFixturator::new(::fixt::Empty)
            .next()
            .unwrap();
        post_commit_invocation.zome = TestWasm::PostCommitSuccess.into();

        let result = ribosome
            .run_post_commit(host_access, post_commit_invocation)
            .unwrap();
        assert_eq!(result, ());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "test_utils")]
    async fn post_commit_test_volley() -> anyhow::Result<()> {
        observability::test_run().ok();
        let (dna_file, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::PostCommitVolley])
            .await
            .unwrap();

        let alice_pubkey = fixt!(AgentPubKey, Predictable, 0);
        let bob_pubkey = fixt!(AgentPubKey, Predictable, 1);

        let mut dna_store = MockDnaStore::new();
        dna_store.expect_add_dnas::<Vec<_>>().return_const(());
        dna_store.expect_add_entry_defs::<Vec<_>>().return_const(());
        dna_store.expect_add_dna().return_const(());
        dna_store
            .expect_get()
            .return_const(Some(dna_file.clone().into()));
        dna_store
            .expect_get_entry_def()
            .return_const(EntryDef::default_with_id("thing"));

        let mut conductor =
            SweetConductor::from_builder(ConductorBuilder::with_mock_dna_store(dna_store)).await;

        let apps = conductor
            .setup_app_for_agents(
                "app-",
                &[alice_pubkey.clone(), bob_pubkey.clone()],
                &[dna_file.into()],
            )
            .await
            .unwrap();

        let ((alice,), (bobbo,)) = apps.into_tuples();
        let alice = alice.zome(TestWasm::PostCommitVolley);
        let bobbo = bobbo.zome(TestWasm::PostCommitVolley);

        let _set_access: () = conductor.call::<_, (), _>(&alice, "set_access", ()).await;

        let _set_access: () = conductor.call::<_, (), _>(&bobbo, "set_access", ()).await;

        let _ping: HeaderHash = conductor.call(&alice, "ping", bob_pubkey).await;

        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        let alice_query: Vec<Element> = conductor.call(&alice, "query", ()).await;

        assert_eq!(alice_query.len(), 5);

        let bob_query: Vec<Element> = conductor.call(&bobbo, "query", ()).await;

        assert_eq!(bob_query.len(), 4);

        Ok(())
    }
}
