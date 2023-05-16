use demo_app::app::create_demo_config;
use demo_app::app::{DefaultPrivateKey, NativeAppRunner};
use jupiter::da_service::{CelestiaService, DaServiceConfig};
use jupiter::types::NamespaceId;
use jupiter::verifier::CelestiaVerifier;
use jupiter::verifier::RollupParams;
use risc0_adapter::host::Risc0Host;
use sovereign_db::ledger_db::LedgerDB;
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
    let da_service = CelestiaService::new(
        DaServiceConfig {
            celestia_rpc_auth_token: CELESTIA_NODE_AUTH_TOKEN.to_string(),
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
    let genesis_config = create_demo_config(
        100000000,
        &DefaultPrivateKey::generate(),
        &DefaultPrivateKey::generate(),
    );
    demo.init_chain(genesis_config);
    demo.begin_slot(Default::default());
    let (prev_state_root, _, _) = demo.end_slot();
    let mut prev_state_root = prev_state_root.0;

    // Request data from the DA layer and apply it to the demo app
    for height in START_HEIGHT.. {
        println!(
            "Requesting data for height {} and prev_state_root 0x{}",
            height,
            hex::encode(&prev_state_root)
        );
        let filtered_block = da_service.get_finalized_at(height).await?;
        let header = filtered_block.header.clone();
        let (blob_txs, inclusion_proof, completeness_proof) =
            da_service.extract_relevant_txs_with_proof(filtered_block);
        assert!(da_verifier
            .verify_relevant_tx_list(&header, &blob_txs, inclusion_proof, completeness_proof)
            .is_ok());
        println!("Received {} blobs", blob_txs.len());

        demo.begin_slot(Default::default());
        for blob in blob_txs.clone() {
            let receipts = demo.apply_blob(blob, None);
            for receipt in receipts {
                l
            }
        }
        let (next_state_root, _witness, _) = demo.end_slot();
        prev_state_root = next_state_root.0;
    }

    Ok(())
}
