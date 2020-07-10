use crate::core::ribosome::error::{RibosomeError, RibosomeResult};
use crate::core::{
    ribosome::{HostContext, RibosomeT},
    state::metadata::{LinkMetaKey, LinkMetaVal},
    workflow::InvokeZomeWorkspace,
};
use futures::future::FutureExt;
use holochain_state::error::DatabaseResult;
use holochain_zome_types::link::Link;
use holochain_zome_types::GetLinksInput;
use holochain_zome_types::GetLinksOutput;
use must_future::MustBoxFuture;
use std::convert::TryInto;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get_links<'a>(
    ribosome: Arc<impl RibosomeT>,
    host_context: Arc<HostContext>,
    input: GetLinksInput,
) -> RibosomeResult<GetLinksOutput> {
    let (base_address, tag) = input.into_inner();

    let base_address = base_address.try_into()?;

    // Get zome id
    let zome_id: holochain_types::header::ZomeId = match ribosome
        .dna_file()
        .dna
        .zomes
        .iter()
        .position(|(name, _)| name == &host_context.zome_name)
    {
        Some(index) => holochain_types::header::ZomeId::from(index as u8),
        None => Err(RibosomeError::ZomeNotExists(host_context.zome_name.clone()))?,
    };

    let call =
        |workspace: &'a InvokeZomeWorkspace| -> MustBoxFuture<'a, DatabaseResult<Vec<LinkMetaVal>>> {
            async move {
                let cascade = workspace.cascade();

                // Create the key
                let key = match tag.as_ref() {
                    Some(tag) => LinkMetaKey::BaseZomeTag(&base_address, zome_id, tag),
                    None => LinkMetaKey::BaseZome(&base_address, zome_id),
                };

                // Get te links from the dht
                cascade
                    .dht_get_links(&key)
                    .await
            }
            .boxed()
            .into()
        };

    let links = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        unsafe { host_context.workspace.apply_ref(call).await }
    })??;

    let links: Vec<Link> = links.into_iter().map(|l| l.into_link()).collect();

    Ok(GetLinksOutput::new(links.into()))
}

#[cfg(test)]
pub mod wasm_test {
    use crate::core::queue_consumer::TriggerSender;
    use crate::core::state::workspace::Workspace;
    use crate::core::workflow::integrate_dht_ops_workflow::{
        integrate_dht_ops_workflow, IntegrateDhtOpsWorkspace,
    };
    use crate::core::workflow::produce_dht_ops_workflow::{
        produce_dht_ops_workflow, ProduceDhtOpsWorkspace,
    };
    use hdk3::prelude::link::LinkTag;
    use hdk3::prelude::*;
    use holo_hash_core::HoloHashCore;
    use holo_hash_core::HoloHashCoreHash;
    use holochain_state::env::ReadManager;
    use holochain_wasm_test_utils::TestWasm;
    use test_wasm_common::*;

    #[tokio::test(threaded_scheduler)]
    async fn ribosome_entry_hash_path_ls() {
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;

        {
            let reader = env_ref.reader().unwrap();
            let mut workspace =
                crate::core::workflow::InvokeZomeWorkspace::new(&reader, &dbs).unwrap();

            // commits fail validation if we don't do genesis
            crate::core::workflow::fake_genesis(&mut workspace.source_chain)
                .await
                .unwrap();

            // touch foo/bar twice to ensure idempotency
            let _: () = {
                let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);
                crate::call_test_ribosome!(
                    raw_workspace,
                    TestWasm::HashPath,
                    "touch",
                    TestString::from("foo/bar".to_string())
                )
            };
            let _: () = {
                let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);
                crate::call_test_ribosome!(
                    raw_workspace,
                    TestWasm::HashPath,
                    "touch",
                    TestString::from("foo/bar".to_string())
                )
            };

            // touch foo/baz
            let _: () = {
                let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);
                crate::call_test_ribosome!(
                    raw_workspace,
                    TestWasm::HashPath,
                    "touch",
                    TestString::from("foo/baz".to_string())
                )
            };

            // Write the database to file
            holochain_state::env::WriteManager::with_commit(&env_ref, |writer| {
                crate::core::state::workspace::Workspace::flush_to_txn(workspace, writer)
            })
            .unwrap();
        };

        // Needs metadata to return get
        {
            use crate::core::state::workspace::Workspace;
            use holochain_state::env::ReadManager;

            // Produce the ops
            let (mut qt, mut rx) = TriggerSender::new();
            {
                let reader = env_ref.reader().unwrap();
                let workspace = ProduceDhtOpsWorkspace::new(&reader, &dbs).unwrap();
                produce_dht_ops_workflow(workspace, env.env.clone().into(), &mut qt)
                    .await
                    .unwrap();
                // await the workflow finishing
                rx.listen().await.unwrap();
            }
            // Integrate the ops
            {
                let reader = env_ref.reader().unwrap();
                let workspace = IntegrateDhtOpsWorkspace::new(&reader, &dbs).unwrap();
                integrate_dht_ops_workflow(workspace, env.env.clone().into(), &mut qt)
                    .await
                    .unwrap();
                rx.listen().await.unwrap();
            }
        }

        let exists_output = {
            let reader = env_ref.reader().unwrap();
            let mut workspace =
                crate::core::workflow::InvokeZomeWorkspace::new(&reader, &dbs).unwrap();

            let output: TestBool = {
                let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);
                crate::call_test_ribosome!(
                    raw_workspace,
                    TestWasm::HashPath,
                    "exists",
                    TestString::from("foo".to_string())
                )
            };

            output
        };

        assert_eq!(TestBool(true), exists_output,);

        let (foo_bar, foo_baz) = {
            let reader = env_ref.reader().unwrap();
            let mut workspace =
                crate::core::workflow::InvokeZomeWorkspace::new(&reader, &dbs).unwrap();

            let foo_bar: holo_hash_core::HoloHashCore = {
                let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);
                crate::call_test_ribosome!(
                    raw_workspace,
                    TestWasm::HashPath,
                    "pwd",
                    TestString::from("foo/bar".to_string())
                )
            };

            let foo_baz: holo_hash_core::HoloHashCore = {
                let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);
                crate::call_test_ribosome!(
                    raw_workspace,
                    TestWasm::HashPath,
                    "pwd",
                    TestString::from("foo/baz".to_string())
                )
            };

            (foo_bar, foo_baz)
        };

        let ls_output = {
            let reader = env_ref.reader().unwrap();
            let mut workspace =
                crate::core::workflow::InvokeZomeWorkspace::new(&reader, &dbs).unwrap();

            let output: holochain_zome_types::link::Links = {
                let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);
                crate::call_test_ribosome!(
                    raw_workspace,
                    TestWasm::HashPath,
                    "ls",
                    TestString::from("foo".to_string())
                )
            };

            output
        };

        let links = ls_output.into_inner();
        assert_eq!(2, links.len());
        assert_eq!(links[0].target, foo_baz,);
        assert_eq!(links[1].target, foo_bar,);
    }

    #[tokio::test(threaded_scheduler)]
    async fn hash_path_anchor_get_anchor() {
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;

        let (anchor_address_one, anchor_address_two) = {
            let reader = env_ref.reader().unwrap();
            let mut workspace =
                crate::core::workflow::InvokeZomeWorkspace::new(&reader, &dbs).unwrap();

            // commits fail validation if we don't do genesis
            crate::core::workflow::fake_genesis(&mut workspace.source_chain)
                .await
                .unwrap();

            // anchor foo bar
            let anchor_address_one: HoloHashCore = {
                let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);
                crate::call_test_ribosome!(
                    raw_workspace,
                    TestWasm::Anchor,
                    "anchor",
                    AnchorInput("foo".to_string(), "bar".to_string())
                )
            };

            assert_eq!(
                anchor_address_one.get_raw().to_vec(),
                vec![
                    251, 69, 127, 15, 3, 239, 206, 255, 170, 38, 140, 9, 220, 244, 93, 184, 84,
                    189, 108, 142, 47, 195, 34, 218, 250, 121, 203, 196, 26, 203, 53, 50, 38, 242,
                    236, 123,
                ],
            );

            // anchor foo baz
            let anchor_address_two: HoloHashCore = {
                let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);
                crate::call_test_ribosome!(
                    raw_workspace,
                    TestWasm::Anchor,
                    "anchor",
                    AnchorInput("foo".to_string(), "baz".to_string())
                )
            };

            assert_eq!(
                anchor_address_two.get_raw().to_vec(),
                vec![
                    40, 91, 139, 255, 25, 187, 26, 6, 97, 212, 166, 244, 73, 56, 176, 96, 53, 235,
                    174, 8, 151, 130, 108, 116, 77, 202, 115, 10, 110, 186, 187, 211, 173, 135, 76,
                    5
                ],
            );

            // Write the database to file
            holochain_state::env::WriteManager::with_commit(&env_ref, |writer| {
                crate::core::state::workspace::Workspace::flush_to_txn(workspace, writer)
            })
            .unwrap();

            (anchor_address_one, anchor_address_two)
        };

        // Needs metadata to return get
        {
            use crate::core::state::workspace::Workspace;
            use holochain_state::env::ReadManager;

            // Produce the ops
            let (mut qt, mut rx) = TriggerSender::new();
            {
                let reader = env_ref.reader().unwrap();
                let workspace = ProduceDhtOpsWorkspace::new(&reader, &dbs).unwrap();
                produce_dht_ops_workflow(workspace, env.env.clone().into(), &mut qt)
                    .await
                    .unwrap();
                // await the workflow finishing
                rx.listen().await.unwrap();
            }
            // Integrate the ops
            {
                let reader = env_ref.reader().unwrap();
                let workspace = IntegrateDhtOpsWorkspace::new(&reader, &dbs).unwrap();
                integrate_dht_ops_workflow(workspace, env.env.clone().into(), &mut qt)
                    .await
                    .unwrap();
                rx.listen().await.unwrap();
            }
        }

        let get_output = {
            let reader = env_ref.reader().unwrap();
            let mut workspace =
                crate::core::workflow::InvokeZomeWorkspace::new(&reader, &dbs).unwrap();

            let input = anchor_address_one.clone();
            let output: MaybeAnchor = {
                let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);
                crate::call_test_ribosome!(raw_workspace, TestWasm::Anchor, "get_anchor", input)
            };

            output
        };

        assert_eq!(
            MaybeAnchor(Some(Anchor {
                anchor_type: "foo".into(),
                anchor_text: Some("bar".into()),
            })),
            get_output,
        );

        let list_anchor_type_addresses_output = {
            let reader = env_ref.reader().unwrap();
            let mut workspace =
                crate::core::workflow::InvokeZomeWorkspace::new(&reader, &dbs).unwrap();

            let output: Hashes = {
                let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);
                crate::call_test_ribosome!(
                    raw_workspace,
                    TestWasm::Anchor,
                    "list_anchor_type_addresses",
                    ()
                )
            };

            output
        };

        // should be 1 anchor type, "foo"
        assert_eq!(list_anchor_type_addresses_output.0.len(), 1,);
        assert_eq!(
            (list_anchor_type_addresses_output.0)[0].get_raw().to_vec(),
            vec![
                46, 187, 74, 48, 90, 73, 153, 38, 193, 172, 241, 90, 224, 154, 107, 253, 214, 55,
                229, 101, 197, 18, 128, 240, 62, 161, 32, 217, 225, 88, 33, 22, 35, 133, 149, 209
            ],
        );

        let list_anchor_addresses_output = {
            let reader = env_ref.reader().unwrap();
            let mut workspace =
                crate::core::workflow::InvokeZomeWorkspace::new(&reader, &dbs).unwrap();

            let output: Hashes = {
                let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);
                crate::call_test_ribosome!(
                    raw_workspace,
                    TestWasm::Anchor,
                    "list_anchor_addresses",
                    TestString("foo".into())
                )
            };

            output
        };

        // should be 2 anchors under "foo" sorted by hash
        assert_eq!(list_anchor_addresses_output.0.len(), 2,);
        assert_eq!(
            (list_anchor_addresses_output.0)[0].get_raw().to_vec(),
            anchor_address_two.get_raw().to_vec(),
        );
        assert_eq!(
            (list_anchor_addresses_output.0)[1].get_raw().to_vec(),
            anchor_address_one.get_raw().to_vec(),
        );

        let list_anchor_tags_output = {
            let reader = env_ref.reader().unwrap();
            let mut workspace =
                crate::core::workflow::InvokeZomeWorkspace::new(&reader, &dbs).unwrap();

            let output: LinkTags = {
                let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);
                crate::call_test_ribosome!(
                    raw_workspace,
                    TestWasm::Anchor,
                    "list_anchor_tags",
                    TestString("foo".into())
                )
            };

            output
        };

        assert_eq!(
            LinkTags(vec![LinkTag(vec![104, 100, 107, 46, 112, 97, 116, 104,],),],),
            list_anchor_tags_output,
        );
    }
}
