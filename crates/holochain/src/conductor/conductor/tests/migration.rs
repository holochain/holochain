use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_cell_agent() {
    let mut conductor = SweetConductor::from_standard_config().await;
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let (cell1,) = conductor
        .setup_app("app1", [&dna])
        .await
        .unwrap()
        .into_tuple();
    let (cell2,) = conductor
        .setup_app("app2", [&dna])
        .await
        .unwrap()
        .into_tuple();

    let (close_hash, _open_hash) = conductor
        .migrate_cell(cell1.cell_id(), cell2.cell_id())
        .await
        .unwrap();

    let actual_close_hash = {
        let old_chain = conductor.get_source_chain(cell1.cell_id()).await.unwrap();
        let records = old_chain.query(ChainQueryFilter::new()).await.unwrap();
        let last = records.last().unwrap();
        match last.action() {
            Action::CloseChain(a) => {
                assert_eq!(
                    a.new_target,
                    Some(MigrationTarget::Agent(cell2.agent_pubkey().clone()))
                );
            }
            _ => unreachable!("unexpected action type"),
        }
        last.action_address().clone()
    };

    assert_eq!(close_hash, actual_close_hash);

    {
        let new_chain = conductor.get_source_chain(cell2.cell_id()).await.unwrap();
        let records = new_chain.query(ChainQueryFilter::new()).await.unwrap();
        let last = records.last().unwrap();
        match last.action() {
            Action::OpenChain(a) => {
                assert_eq!(
                    a.prev_target,
                    MigrationTarget::Agent(cell1.agent_pubkey().clone())
                );
                assert_eq!(a.close_hash, close_hash);
            }
            _ => unreachable!("unexpected action type"),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_cell_dna() {
    let config = SweetConductorConfig::standard().no_dpki();
    let mut conductor = SweetConductor::from_config(config).await;
    let (dna1, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let (dna2, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let agent = SweetAgents::one(conductor.keystore()).await;

    let (cell1,) = conductor
        .setup_app_for_agent("app1", agent.clone(), [&dna1])
        .await
        .unwrap()
        .into_tuple();
    let (cell2,) = conductor
        .setup_app_for_agent("app2", agent.clone(), [&dna2])
        .await
        .unwrap()
        .into_tuple();
    let (close_hash, _open_hash) = conductor
        .migrate_cell(cell1.cell_id(), cell2.cell_id())
        .await
        .unwrap();

    let actual_close_hash = {
        let old_chain = conductor.get_source_chain(cell1.cell_id()).await.unwrap();
        let records = old_chain.query(ChainQueryFilter::new()).await.unwrap();
        let last = records.last().unwrap();
        match last.action() {
            Action::CloseChain(a) => {
                assert_eq!(
                    a.new_target,
                    Some(MigrationTarget::Dna(dna2.dna_hash().clone()))
                );
            }
            _ => unreachable!("unexpected action type"),
        }
        last.action_address().clone()
    };

    assert_eq!(close_hash, actual_close_hash);

    {
        let new_chain = conductor.get_source_chain(cell2.cell_id()).await.unwrap();
        let records = new_chain.query(ChainQueryFilter::new()).await.unwrap();
        let last = records.last().unwrap();
        match last.action() {
            Action::OpenChain(a) => {
                assert_eq!(a.prev_target, MigrationTarget::Dna(dna1.dna_hash().clone()));
                assert_eq!(a.close_hash, close_hash);
            }
            _ => unreachable!("unexpected action type"),
        }
    }
}
