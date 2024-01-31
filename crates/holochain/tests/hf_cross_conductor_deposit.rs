use holo_hash::{ActionHash, AgentPubKey, AgentPubKeyB64, EntryHashB64};
use holochain::sweettest::{SweetAgents, SweetApp, SweetConductor, SweetDnaFile};
use holochain_serialized_bytes::prelude::{Serialize, SerializedBytes};
use holochain_types::prelude::{
    AppBundle, AppBundleSource, AppManifestCurrentBuilder, AppRoleDnaManifest, AppRoleManifest,
    CellProvisioning, DnaLocation, InstallAppPayload,
};
use holochain_zome_types::{DnaModifiersOpt, MembraneProof, Record, Timestamp};
use holofuel_types::fuel::Fuel;
use reserves::gets::Reserve;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};
use transactor::{
    entries::transaction::signal_types::CounterSigningResponse,
    externs::{
        post::AcceptTx,
        reserve::{DepositInput, ReservePayload},
    },
    return_types::Transaction,
};
use transactor_integrity::{
    reserves::{ReserveProof, ReserveSalePrice, ReserveSetting},
    states::ledger::Ledger,
};

#[derive(Debug, Serialize)]
struct MembraneProofPayload {
    role: String,
    record_locator: String,
    registered_agent: AgentPubKeyB64,
}

async fn generate_membrane_proof(
    conductor: &SweetConductor,
    mem_proof_app: &SweetApp,
    agent_pub_key: &AgentPubKey,
    mem_proof_type: Option<String>,
) -> MembraneProof {
    const ZOME_NAME_CODE_GENERATOR: &str = "code-generator";
    let payload = MembraneProofPayload {
        role: mem_proof_type.unwrap_or_else(|| "holofuel".to_string()),
        record_locator: "RECORD_LOCATOR".to_string(),
        registered_agent: AgentPubKeyB64::from(agent_pub_key.to_owned()),
    };
    let membrane_proof: Record = conductor
        .call(
            &mem_proof_app.cells()[0].zome(ZOME_NAME_CODE_GENERATOR),
            "make_proof",
            payload,
        )
        .await;
    let serialized_membrane_proof = SerializedBytes::try_from(membrane_proof).unwrap();
    Arc::new(serialized_membrane_proof)
}

#[tokio::test(flavor = "multi_thread")]
async fn hf_cross_conductor_deposit() {
    const ZOME_NAME_TRANSACTOR: &str = "transactor";
    const ZOME_NAME_RESERVES: &str = "reserves";
    const ROLE_NAME: &str = "hf-test";

    let joining_code_factory_role_name = "jcf".to_string();
    let joining_code_factory_dna_path =
        Path::new("/home/thetasinner/source/holo/joining-code-happ/joining-code-factory.dna");
    let holofuel_dna_path = Path::new("/home/thetasinner/source/holo/holofuel/holofuel.dna");

    // create membrane proof generator
    println!("creating membrane proof generator");
    let mut membrane_proof_conductor = SweetConductor::from_standard_config().await;
    let membrane_proof_dna = SweetDnaFile::from_bundle(joining_code_factory_dna_path)
        .await
        .unwrap();
    let membrane_proof_role_with_dna = (joining_code_factory_role_name, membrane_proof_dna);
    let membrane_proof_generator = membrane_proof_conductor
        .setup_app("membrane_proof_generator", &[membrane_proof_role_with_dna])
        .await
        .unwrap();

    // create reserve
    println!("creating reserve");
    let reserve_conductor = SweetConductor::from_standard_config().await;
    let reserve_agent_pub_key = SweetAgents::one(reserve_conductor.keystore()).await;
    let reserve_membrane_proof = generate_membrane_proof(
        &membrane_proof_conductor,
        &membrane_proof_generator,
        &reserve_agent_pub_key,
        Some("holofuel_reserve".to_string()),
    )
    .await;
    let mut reserve_membrane_proofs = HashMap::new();
    reserve_membrane_proofs.insert(ROLE_NAME.to_string(), reserve_membrane_proof);

    let holofuel_properties = format!(
        "holo_agent_override: \"{}\"
enable_reserves: true",
        AgentPubKeyB64::from(membrane_proof_generator.agent().to_owned())
    );
    let app_roles = AppRoleManifest {
        name: ROLE_NAME.into(),
        dna: AppRoleDnaManifest {
            location: Some(DnaLocation::Path(holofuel_dna_path.to_path_buf())),
            modifiers: DnaModifiersOpt::none()
                .with_properties(serde_yaml::from_str(&holofuel_properties).unwrap()),
            installed_hash: None,
            clone_limit: 0,
        },
        provisioning: Some(CellProvisioning::Create { deferred: false }),
    };
    let manifest = AppManifestCurrentBuilder::default()
        .name("holofuel".to_string())
        .description(None)
        .roles(vec![app_roles])
        .build()
        .unwrap();
    let bundle = AppBundle::new(manifest.clone().into(), vec![], PathBuf::from("."))
        .await
        .unwrap();
    let reserve_app = reserve_conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            source: AppBundleSource::Bundle(bundle),
            agent_key: reserve_agent_pub_key.clone(),
            membrane_proofs: reserve_membrane_proofs,
            installed_app_id: None,
            network_seed: None,
        })
        .await
        .unwrap();
    let reserve_app = reserve_conductor
        .clone()
        .enable_app(reserve_app.id().to_owned())
        .await
        .unwrap();
    let reserve_cell = reserve_app.0.all_cells().next().unwrap();

    // set up reserve
    println!("setting up reserve");
    let reserve_sign_key = reserve_conductor
        .keystore()
        .new_sign_keypair_random()
        .await
        .unwrap();
    let reserve_payload = ReserveSetting {
        external_reserve_currency: "HOT".to_string(),
        external_account_number: "hot-acount-#".to_string(),
        external_signing_key: reserve_sign_key.get_raw_32().try_into().unwrap(),
        default_promise_expiry: Duration::new(0, 0),
        note: None,
        min_external_currency_tx_size: "1".to_string(),
        max_external_currency_tx_size: "1000".to_string(),
    };
    let _reserve: Reserve = reserve_conductor
        .easy_call_zome(
            reserve_app.0.agent_key(),
            None,
            reserve_cell.to_owned(),
            ZOME_NAME_RESERVES,
            "register_reserve_account",
            reserve_payload,
        )
        .await
        .unwrap();

    const LATEST_UNIT_PRICE: i128 = 1; // this means 1 HOT = 1 HF
    let set_sale_payload = ReserveSalePrice {
        latest_unit_price: Fuel::from_str(&LATEST_UNIT_PRICE.to_string()).unwrap(),
        inputs_used: vec![],
    };
    let price_set: ActionHash = reserve_conductor
        .easy_call_zome(
            reserve_app.0.agent_key(),
            None,
            reserve_cell.to_owned(),
            ZOME_NAME_RESERVES,
            "set_sale_price",
            set_sale_payload,
        )
        .await
        .unwrap();
    println!("price set {:?}", price_set);

    // init transactor zome
    let _: Ledger = reserve_conductor
        .clone()
        .easy_call_zome(
            &reserve_agent_pub_key,
            None,
            reserve_cell.to_owned(),
            ZOME_NAME_TRANSACTOR,
            "get_ledger",
            (),
        )
        .await
        .unwrap();

    // set up agent accounts
    let agents_conductor = SweetConductor::from_standard_config().await;
    const AGENTS_PER_CONDUCTOR: usize = 1;
    let agent_keys = SweetAgents::get(agents_conductor.keystore(), AGENTS_PER_CONDUCTOR).await;
    let mut agent_membrane_proofs = vec![];
    for agent_key in agent_keys.iter() {
        let membrane_proof = generate_membrane_proof(
            &membrane_proof_conductor,
            &membrane_proof_generator,
            agent_key,
            None,
        )
        .await;
        agent_membrane_proofs.push(membrane_proof);
    }

    let mut agent_apps = vec![];
    for (index, agent_key) in agent_keys.iter().enumerate() {
        let membrane_proof = agent_membrane_proofs[index].clone();
        let mut membrane_proofs = HashMap::new();
        membrane_proofs.insert(ROLE_NAME.to_string(), membrane_proof);
        let app_bundle = AppBundle::new(manifest.clone().into(), vec![], PathBuf::from("."))
            .await
            .unwrap();
        let app_id = format!("holofuel-{}", agent_key);
        let stopped_app = agents_conductor
            .clone()
            .install_app_bundle(InstallAppPayload {
                source: AppBundleSource::Bundle(app_bundle),
                agent_key: agent_key.to_owned(),
                membrane_proofs,
                installed_app_id: Some(app_id),
                network_seed: None,
            })
            .await
            .unwrap();
        let agent_app = agents_conductor
            .clone()
            .enable_app(stopped_app.id().to_owned())
            .await
            .unwrap();
        println!("hello here {:?}", agent_app);
        let _: Ledger = agents_conductor
            .easy_call_zome(
                agent_key,
                None,
                agent_app.0.all_cells().next().unwrap().clone(),
                ZOME_NAME_TRANSACTOR,
                "get_ledger",
                (),
            )
            .await
            .unwrap();
        agent_apps.push(agent_app);
    }

    // // familiarize all agents
    // // SweetConductor::exchange_peer_info([&reserve_conductor, &agents_conductor]).await;

    // let the deposits begin
    const INITIAL_BALANCE: i128 = 100;
    let amount = INITIAL_BALANCE.to_string();
    let ext_amt_transferred = INITIAL_BALANCE * LATEST_UNIT_PRICE;
    let reserve_proof = ReserveProof {
        ext_amt_transferred: Fuel::from_str(&ext_amt_transferred.to_string()).unwrap(),
        nonce: "nonce-string".to_string(),
        reserve_sales_price: Fuel::from_str(&LATEST_UNIT_PRICE.to_string()).unwrap(),
        ext_tx_id: None,
    };
    let data_to_sign = SerializedBytes::try_from(reserve_proof.clone())
        .unwrap()
        .bytes()
        .to_owned();
    let signature = reserve_conductor
        .keystore()
        .sign(
            reserve_sign_key.clone(),
            data_to_sign.into_boxed_slice().into(),
        )
        .await
        .unwrap();
    for (index, agent_key) in agent_keys.iter().enumerate() {
        let start = Instant::now();
        let deposit_input = DepositInput {
            receiver: AgentPubKeyB64::from(agent_key.clone()),
            amount: amount.clone(),
            reserve_payload: ReservePayload {
                details: reserve_proof.clone(),
                signature: signature.clone(),
            },
            expiration_date: Some(Timestamp::max()),
            url: None,
            note: None,
        };
        println!("depositing to agent {} with key {:?}", index, agent_key);
        let agent_cell = agent_apps[index].0.all_cells().next().unwrap();
        let tx: Transaction = reserve_conductor
            .clone()
            .easy_call_zome(
                &reserve_agent_pub_key,
                None,
                reserve_cell.to_owned(),
                ZOME_NAME_TRANSACTOR,
                "deposit",
                deposit_input,
            )
            .await
            .unwrap();
        println!("tx {:?}", tx);

        let accept_tx_payload = AcceptTx {
            address: tx.id.clone(),
            expiration_date: Some(Timestamp::max()),
        };
        let accepted_tx_hash: EntryHashB64 = agents_conductor
            .easy_call_zome(
                agent_key,
                None,
                agent_cell.clone(),
                ZOME_NAME_TRANSACTOR,
                "accept_transaction",
                accept_tx_payload,
            )
            .await
            .unwrap();
        println!("accept tx hash {:?}", accepted_tx_hash);

        let complete_tx_response: CounterSigningResponse = agents_conductor
            .easy_call_zome(
                agent_key,
                None,
                agent_cell.clone(),
                ZOME_NAME_TRANSACTOR,
                "complete_transactions",
                accepted_tx_hash,
            )
            .await
            .unwrap();
        let elapsed = Instant::now() - start;
        println!(
            "complete tx response {:?}, took {:?}\n\n",
            complete_tx_response, elapsed
        );
    }
}
