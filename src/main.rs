use context::DemoContext;
use jsonrpsee::http_client::HeaderMap;
use jupiter::{
    da_app::{CelestiaApp, TmHash},
    da_service::{CelestiaService, FilteredCelestiaBlock},
};
use sha2::{Digest, Sha256};
use sov_app_template::{AppTemplate, Batch};
use sov_state::ProverStorage;
use sovereign_db::{
    ledger_db::{LedgerDB, SlotCommitBuilder},
    schema::types::{
        BatchNumber, DbBytes, DbHash, EventNumber, Status, StoredBatch, StoredSlot,
        StoredTransaction, TxNumber,
    },
};
use sovereign_sdk::{
    da::BlobTransactionTrait,
    serial::Encode,
    services::da::{DaService, SlotData},
};
use sovereign_sdk::{da::DaLayerTrait, stf::StateTransitionFunction};
use sovereign_sdk::{db::SlotStore, serial::Decode};

use tracing::Level;
use tx_verifier_impl::DemoAppTxVerifier;

use crate::{
    data_generation::QueryGenerator, helpers::run_query, runtime::Runtime,
    tx_hooks_impl::DemoAppTxHooks,
};

mod context;
mod data_generation;
mod helpers;
mod runtime;
mod tx_hooks_impl;
mod tx_verifier_impl;

type C = DemoContext;
type DemoApp = AppTemplate<C, DemoAppTxVerifier<C>, Runtime<C>, DemoAppTxHooks<C>>;
const CELESTIA_NODE_AUTH_TOKEN: &'static str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJBbGxvdyI6WyJwdWJsaWMiLCJyZWFkIiwid3JpdGUiLCJhZG1pbiJdfQ.NuLolaPIYqxI1L6ISDq3zZ9aPeHFVw5wHt8DQrN3ct8";

const START_HEIGHT: u64 = HEIGHT_OF_FIRST_TXS - 5;
// I sent 8 demo election transactions at height 293686, generated using the demo app data generator
const HEIGHT_OF_FIRST_TXS: u64 = 293686;
pub fn default_celestia_service() -> CelestiaService {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Authorization",
        format!("Bearer {}", CELESTIA_NODE_AUTH_TOKEN)
            .parse()
            .unwrap(),
    );
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .set_headers(headers)
        .max_request_body_size(1024 * 1024 * 100) // 100 MB
        .build("http://localhost:11111/")
        .unwrap();
    CelestiaService::with_client(client)
}

fn sha2(data: impl AsRef<[u8]>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data.as_ref());
    hasher.finalize().into()
}

const DATA_DIR_LOCATION: &'static str = "demo_data";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|_err| eprintln!("Unable to set global default subscriber"))
        .expect("Cannot fail to set subscriber");
    let cel_service = default_celestia_service();
    let ledger_db = LedgerDB::<FilteredCelestiaBlock>::with_path(DATA_DIR_LOCATION).unwrap();
    let storage = ProverStorage::with_path(DATA_DIR_LOCATION).unwrap();
    let mut demo = DemoApp::new(
        storage.clone(),
        Runtime::new(),
        DemoAppTxVerifier::new(),
        DemoAppTxHooks::new(),
    );
    let da_app = CelestiaApp {
        db: ledger_db.clone(),
    };
    let mut item_numbers = ledger_db.get_next_items_numbers();
    if item_numbers.slot_number == 1 {
        print!("No history detected. Initializing chain...");
        demo.init_chain(());
        println!("Done.");
    } else {
        println!("Chain is already initialized. Skipping initialization.");
    }
    let last_slot_processed_before_shutdown = item_numbers.slot_number - 1;

    println!("Beginning sync from da slot {}...", START_HEIGHT);
    for i in 0.. {
        let height = START_HEIGHT + i;
        if last_slot_processed_before_shutdown > i {
            println!("Slot at {} has already been processed! Skipping", height);
            continue;
        } else {
            println!("Processing slot at {}", height);
        }

        let filtered_block: FilteredCelestiaBlock = cel_service.get_finalized_at(height).await?;
        let next_block_hash = TmHash(filtered_block.header.header.hash());
        let slot_hash = filtered_block.hash();
        let slot_extra_data = filtered_block.extra_data_for_storage();

        ledger_db.insert(slot_hash.clone(), filtered_block);
        let mut data_to_persist = SlotCommitBuilder::default();
        let batches = da_app.get_relevant_txs(&next_block_hash);

        demo.begin_slot();
        let num_batches = batches.len();
        println!("  Found {} batches.", num_batches);
        for raw_batch in batches {
            let mut data = raw_batch.data();
            let batch = match Batch::decode(&mut data) {
                Ok(batch) => batch,
                Err(e) => {
                    println!("    Error decoding batch: {}. Skipping.", e);
                    continue;
                }
            };
            let batch_hash = sha2(batch.encode_to_vec());
            let tx_start = item_numbers.tx_number;
            let num_txs = batch.txs.len();
            let mut batch_to_store = StoredBatch {
                hash: DbBytes::new(batch_hash.to_vec()),
                extra_data: DbBytes::new(raw_batch.sender.as_ref().to_vec()),
                txs: TxNumber(tx_start)..TxNumber(tx_start + num_txs as u64),
                status: Status::Skipped,
            };
            item_numbers.tx_number += num_txs as u64;
            print!("    Applying batch of {} transactions...", num_txs);

            match demo.apply_batch(batch.clone(), raw_batch.sender().as_ref(), None) {
                Ok(events) => {
                    println!(" Done!");
                    batch_to_store.status = Status::Applied;
                    data_to_persist.txs.extend(batch.txs.into_iter().map(|tx| {
                        let start_event_number = item_numbers.event_number;
                        let end_event_number = start_event_number + events.len() as u64;
                        item_numbers.event_number = end_event_number;
                        StoredTransaction {
                            hash: DbHash::new(sha2(&tx.data[..]).to_vec()),
                            events: EventNumber(start_event_number)..EventNumber(end_event_number),
                            data: DbBytes::new(tx.data),
                            status: Status::Applied,
                        }
                    }));
                    data_to_persist.batches.push(batch_to_store);

                    data_to_persist.events.extend(events.into_iter());
                }
                Err(e) => {
                    println!(
                        " Uh-oh! Failed to apply batch. Applying consensus set update {:?}",
                        e
                    );
                }
            }
        }

        demo.end_slot();
        data_to_persist.slot_data = Some(StoredSlot {
            hash: DbHash::new(slot_hash.to_vec()),
            extra_data: DbBytes::new(slot_extra_data),
            batches: BatchNumber(item_numbers.batch_number)
                ..BatchNumber(item_numbers.batch_number + num_batches as u64),
        });
        item_numbers.batch_number += num_batches as u64;
        ledger_db.commit_slot(data_to_persist.finalize()?)?;

        println!(
            "Current state: {}",
            run_query(
                &mut demo.runtime,
                QueryGenerator::generate_query_election_message(),
                storage.clone(),
            )
        );
    }

    Ok(())
}
