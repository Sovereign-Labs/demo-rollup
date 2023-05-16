use demo_app::app::create_demo_genesis_config;
use demo_app::app::{DefaultPrivateKey, NativeAppRunner};
use jupiter::da_service::{CelestiaService, DaServiceConfig};
use jupiter::types::NamespaceId;
use jupiter::verifier::CelestiaVerifier;
use jupiter::verifier::RollupParams;
use risc0_adapter::host::Risc0Host;
use sovereign_db::ledger_db::{LedgerDB, SlotCommit};
use sovereign_sdk::da::DaVerifier;
use sovereign_sdk::services::da::DaService;
use sovereign_sdk::stf::{StateTransitionFunction, StateTransitionRunner};
use tracing::Level;

const CELESTIA_NODE_AUTH_TOKEN: &'static str = "";

// I sent 8 demo election transactions at height 293686, generated using the demo app data generator
const HEIGHT_OF_FIRST_TXS: u64 = 293686;
const START_HEIGHT: u64 = HEIGHT_OF_FIRST_TXS - 1;
const DATA_DIR_LOCATION: &'static str = "demo_data";
const ROLLUP_NAMESPACE: NamespaceId = NamespaceId([115, 111, 118, 45, 116, 101, 115, 116]);

pub fn intialize_ledger() -> LedgerDB {
    let ledger_db = LedgerDB::with_path(DATA_DIR_LOCATION).expect("Ledger DB failed to open");
    ledger_db
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Initializing logging
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::WARN)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|_err| eprintln!("Unable to set global default subscriber"))
        .expect("Cannot fail to set subscriber");

    let ledger_db = intialize_ledger();

    // Initialize the Celestia service
    let mut demo_runner = NativeAppRunner::<Risc0Host>::new(DATA_DIR_LOCATION);
    let celestia_rpc_auth_token = if !CELESTIA_NODE_AUTH_TOKEN.is_empty() {
        CELESTIA_NODE_AUTH_TOKEN.to_string()
    } else {
        std::env::var("CELESTIA_NODE_AUTH_TOKEN")
            .expect("please set CELESTIA_NODE_AUTH_TOKEN environment variable")
    };
    let da_service = CelestiaService::new(
        DaServiceConfig {
            celestia_rpc_auth_token,
            celestia_rpc_address: "http://localhost:11111/".into(),
            max_celestia_response_body_size: 1024 * 1024 * 100,
        },
        RollupParams {
            namespace: ROLLUP_NAMESPACE,
        },
    );
    let da_verifier = CelestiaVerifier::new(RollupParams {
        namespace: ROLLUP_NAMESPACE,
    });

    // Initialize the demo app
    let demo = demo_runner.inner_mut();
    let sequencer_private_key = DefaultPrivateKey::generate();
    let genesis_config = create_demo_genesis_config(
        100000000,
        sequencer_private_key.default_address::<sha2::Sha256>(),
        vec![
            99, 101, 108, 101, 115, 116, 105, 97, 49, 122, 102, 118, 114, 114, 102, 97, 113, 57,
            117, 100, 54, 103, 57, 116, 52, 107, 122, 109, 115, 108, 112, 102, 50, 52, 121, 115,
            97, 120, 113, 102, 110, 122, 101, 101, 53, 119, 57,
        ],
        &sequencer_private_key,
        &sequencer_private_key,
    );

    let item_numbers = ledger_db.get_next_items_numbers();
    let last_slot_processed_before_shutdown = item_numbers.slot_number - 1;
    if last_slot_processed_before_shutdown == 0 {
        print!("No history detected. Initializing chain...");
        demo.init_chain(genesis_config);
        println!("Done.");
    } else {
        println!("Chain is already initialized. Skipping initialization.");
    }

    demo.begin_slot(Default::default());
    let (prev_state_root, _, _) = demo.end_slot();
    let mut prev_state_root = prev_state_root.0;

    let start_height = START_HEIGHT + last_slot_processed_before_shutdown;
    // Request data from the DA layer and apply it to the demo app
    for height in start_height..start_height + 10 {
        println!(
            "Requesting data for height {} and prev_state_root 0x{}",
            height,
            hex::encode(&prev_state_root)
        );
        let filtered_block = da_service.get_finalized_at(height).await?;
        let header = filtered_block.header.clone();
        let (blob_txs, inclusion_proof, completeness_proof) =
            da_service.extract_relevant_txs_with_proof(filtered_block.clone());
        assert!(da_verifier
            .verify_relevant_tx_list(&header, &blob_txs, inclusion_proof, completeness_proof)
            .is_ok());
        println!("Received {} blobs", blob_txs.len());

        let mut data_to_commit = SlotCommit::new(filtered_block);
        demo.begin_slot(Default::default());
        for blob in blob_txs.clone() {
            let receipts = demo.apply_blob(blob, None);
            data_to_commit.add_batch(receipts);
        }
        let (next_state_root, _witness, _) = demo.end_slot();
        ledger_db.commit_slot(data_to_commit)?;
        prev_state_root = next_state_root.0;
    }

    Ok(())
}
