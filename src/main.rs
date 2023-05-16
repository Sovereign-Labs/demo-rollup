use demo_app::app::create_demo_config;
use demo_app::app::NativeAppRunner;
use jupiter::da_service::{CelestiaService, DaServiceConfig};
use jupiter::types::NamespaceId;
use jupiter::verifier::CelestiaVerifier;
use jupiter::verifier::RollupParams;
use sov_modules_api::default_signature::private_key::DefaultPrivateKey;
use sovereign_sdk::da::DaVerifier;
use sovereign_sdk::services::da::{DaService, SlotData};
use sovereign_sdk::stf::{StateTransitionFunction, StateTransitionRunner};
use sov_app_template::SequencerOutcome;
use sovereign_db::{
    ledger_db::{LedgerDB, SlotCommitBuilder},
    schema::types::{
        BatchNumber, DbBytes, EventNumber, Status, StoredBatch, StoredSlot, StoredTransaction,
        TxNumber,
    },
};
use tracing::Level;

const CELESTIA_NODE_AUTH_TOKEN: &'static str = "";

// I sent 8 demo election transactions at height 293686, generated using the demo app data generator
const HEIGHT_OF_FIRST_TXS: u64 = 293686;
const START_HEIGHT: u64 = HEIGHT_OF_FIRST_TXS - 1;
const DATA_DIR_LOCATION: &'static str = "demo_data";
const ROLLUP_NAMESPACE: NamespaceId = NamespaceId([115, 111, 118, 45, 116, 101, 115, 116]);

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Initializing logging
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::WARN)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|_err| eprintln!("Unable to set global default subscriber"))
        .expect("Cannot fail to set subscriber");

    // Initialize the Celestia service
    let ledger_db = LedgerDB::with_path(DATA_DIR_LOCATION).unwrap();
    let mut demo_runner = NativeAppRunner::new(DATA_DIR_LOCATION);
    let celestia_rpc_auth_token = if !CELESTIA_NODE_AUTH_TOKEN.is_empty() {
        CELESTIA_NODE_AUTH_TOKEN.to_string()
    } else {
        std::env::var("CELESTIA_NODE_AUTH_TOKEN").expect("CELESTIA_NODE_AUTH_TOKEN")
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

    // let rpc_ledger = ledger_db.clone();
    // let _rpc_handle = rpc::RpcProvider::start(rpc_ledger, rpc_storage).await?;

    // Initialize the demo app
    let demo = demo_runner.inner_mut();
    let genesis_config = create_demo_config(
        100000000,
        &DefaultPrivateKey::generate(),
        &DefaultPrivateKey::generate(),
    );


    let mut item_numbers = ledger_db.get_next_items_numbers();
    if item_numbers.slot_number == 1 {
        print!("No history detected. Initializing chain...");
        demo.init_chain(genesis_config);
        println!("Done.");
    } else {
        println!("Chain is already initialized. Skipping initialization.");
    }

    demo.begin_slot(Default::default());
    let (prev_state_root, _, _) = demo.end_slot();
    let mut prev_state_root = prev_state_root.0;

    let last_slot_processed_before_shutdown = item_numbers.slot_number - 1;

    let start_height = std::cmp::max(START_HEIGHT,last_slot_processed_before_shutdown);
    // Request data from the DA layer and apply it to the demo app
    for height in start_height.. {
        println!(
            "Requesting data for height {} and prev_state_root 0x{}",
            height,
            hex::encode(&prev_state_root)
        );
        let filtered_block = da_service.get_finalized_at(height).await?;
        let slot_hash = filtered_block.hash();
        let header = filtered_block.header.clone();
        let (blob_txs, inclusion_proof, completeness_proof) =
            da_service.extract_relevant_txs_with_proof(filtered_block);
        assert!(da_verifier
            .verify_relevant_tx_list(&header, &blob_txs, inclusion_proof, completeness_proof)
            .is_ok());
        println!("Received {} blobs", blob_txs.len());


        let mut data_to_persist = SlotCommitBuilder::default();
        demo.begin_slot(Default::default());

        let num_batches = blob_txs.len();
        for blob in blob_txs {
            let sender = blob.sender.as_ref().to_vec();

            let batch_receipt = demo.apply_blob(blob, None);

            let batch_hash = batch_receipt.batch_hash;
            let tx_start = item_numbers.tx_number;
            let num_txs = batch_receipt.tx_receipts.len();

            let mut batch_to_store = StoredBatch {
                sender: sender.clone(),
                hash: batch_hash,
                extra_data: DbBytes::new(sender),
                txs: TxNumber(tx_start)..TxNumber(tx_start + num_txs as u64),
                status: Status::Skipped,
            };

            match batch_receipt.inner {
                SequencerOutcome::Rewarded => {
                    batch_to_store.status = Status::Applied;
                    data_to_persist.batches.push(batch_to_store);
                    data_to_persist.txs.extend(batch_receipt.tx_receipts.into_iter().map(|tx_receipt| {
                        let start_event_number = item_numbers.event_number;
                        let end_event_number = start_event_number + tx_receipt.events.len() as u64;
                        item_numbers.event_number = end_event_number;

                        StoredTransaction {
                            hash: tx_receipt.tx_hash,
                            events: EventNumber(start_event_number)..EventNumber(end_event_number),
                            data: DbBytes::new(tx_receipt.body_to_save.unwrap_or_default()),
                            // TODO: Where to get status?
                            status: Status::Applied,
                        }
                    }));
                }
                SequencerOutcome::Slashed(_) | SequencerOutcome::Ignored =>
                    {
                        println!("Oops, Failed to apply batch")
                    }
            };


            item_numbers.tx_number += num_txs as u64;
        }
        let (next_state_root, _witness, _) = demo.end_slot();
        prev_state_root = next_state_root.0;
        data_to_persist.slot_data = Some(StoredSlot {
            hash: slot_hash,
            extra_data: DbBytes::new(vec![]),
            batches: BatchNumber(item_numbers.batch_number)
                ..BatchNumber(item_numbers.batch_number + num_batches as u64),
        });
    }

    Ok(())
}
