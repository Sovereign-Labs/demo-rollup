use crate::runtime::GenesisConfig;
use bank::TokenConfig;
use jsonrpsee::http_client::HeaderMap;
use jupiter::da_service::{CelestiaService, FilteredCelestiaBlock};
use sha2::{Digest, Sha256};
use sov_app_template::{AppTemplate, Batch};
use sov_modules_api::{mocks::MockContext, Address};
use sov_state::ProverStorage;
use sovereign_db::{
    ledger_db::{LedgerDB, SlotCommitBuilder},
    schema::types::{
        BatchNumber, DbBytes, EventNumber, Status, StoredBatch, StoredSlot, StoredTransaction,
        TxNumber,
    },
};
use sovereign_sdk::serial::Decode;
use sovereign_sdk::services::da::SlotData;
use sovereign_sdk::{da::BlobTransactionTrait, stf::StateTransitionFunction};
use sovereign_sdk::{serial::Encode, services::da::DaService};

use tracing::Level;
use tx_verifier_impl::DemoAppTxVerifier;

use crate::{runtime::Runtime, tx_hooks_impl::DemoAppTxHooks};

mod data_generation;
mod helpers;
mod rpc;
mod runtime;
mod tx_hooks_impl;
mod tx_verifier_impl;

#[derive(Debug, Clone)]
struct Spec;

impl RollupSpec for Spec {
    type SlotData = FilteredCelestiaBlock;

    type Stf = DemoApp;

    type Hasher = Sha256;
}

type C = MockContext;
type DemoApp =
    AppTemplate<C, DemoAppTxVerifier<C>, Runtime<C>, DemoAppTxHooks<C>, GenesisConfig<C>>;
const CELESTIA_NODE_AUTH_TOKEN: &'static str = "";

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
    let sequencer_address: Address = [1u8; 32].into();

    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::WARN)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|_err| eprintln!("Unable to set global default subscriber"))
        .expect("Cannot fail to set subscriber");
    let cel_service = default_celestia_service();
    let ledger_db = LedgerDB::with_path(DATA_DIR_LOCATION).unwrap();
    let storage = ProverStorage::with_path(DATA_DIR_LOCATION).unwrap();
    let mut demo = DemoApp::new(
        storage.clone(),
        Runtime::new(),
        DemoAppTxVerifier::new(),
        DemoAppTxHooks::new(),
        GenesisConfig::new(
            (),
            bank::BankConfig {
                tokens: vec![TokenConfig {
                    token_name: "sovereign".to_string(),
                    address_and_balances: vec![(sequencer_address, 1000)],
                }],
            },
            accounts::AccountConfig { pub_keys: vec![] },
        ),
    );

    let rpc_ledger = ledger_db.clone();
    let rpc_storage = storage.clone();

    let _rpc_handle = rpc::RpcProvider::start(rpc_ledger, rpc_storage).await?;

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
            continue;
        }

        let filtered_block: FilteredCelestiaBlock = cel_service.get_finalized_at(height).await?;
        let slot_hash = filtered_block.hash();
        let mut data_to_persist = SlotCommitBuilder::default();
        let batches = cel_service.extract_relevant_txs(filtered_block);

        demo.begin_slot();
        let num_batches = batches.len();
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
                sender: raw_batch.sender.as_ref().to_vec(),
                hash: batch_hash,
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
                            hash: sha2(&tx.data[..]),
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
            hash: slot_hash,
            extra_data: DbBytes::new(vec![]),
            batches: BatchNumber(item_numbers.batch_number)
                ..BatchNumber(item_numbers.batch_number + num_batches as u64),
        });
        item_numbers.batch_number += num_batches as u64;
        ledger_db.commit_slot(data_to_persist.finalize()?)?;
    }

    Ok(())
}
