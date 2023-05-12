use demo_app::app::create_demo_config;
use demo_app::app::NativeAppRunner;
use jupiter::da_service::{CelestiaService, DaServiceConfig};
use jupiter::types::NamespaceId;
use jupiter::verifier::RollupParams;
use sovereign_sdk::services::da::DaService;
use sovereign_sdk::stf::{StateTransitionFunction, StateTransitionRunner};
use sov_modules_api::{
    default_context::DefaultContext, default_signature::private_key::DefaultPrivateKey, Address,
};
use tracing::Level;


const CELESTIA_NODE_AUTH_TOKEN: &'static str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJBbGxvdyI6WyJwdWJsaWMiLCJyZWFkIiwid3JpdGUiLCJhZG1pbiJdfQ.rr8JrAV2D-7bYMqCwxia7xjpdDbVvVJomMKRA1h-_Ac";

// I sent 8 demo election transactions at height 293686, generated using the demo app data generator
const HEIGHT_OF_FIRST_TXS: u64 = 293686;
const START_HEIGHT: u64 = HEIGHT_OF_FIRST_TXS - 1;
const DATA_DIR_LOCATION: &'static str = "demo_data";


#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let mut demo_runner = NativeAppRunner::new(DATA_DIR_LOCATION);
    let da_service = CelestiaService::new(
        DaServiceConfig {
            celestia_rpc_auth_token: CELESTIA_NODE_AUTH_TOKEN.to_string(),
            celestia_rpc_address: "http://localhost:11111/".into(),
            max_celestia_response_body_size: 1024 * 1024 * 100,
        },
        RollupParams {
            namespace: NamespaceId([115, 111, 118, 45, 116, 101, 115, 116]),
        },
    );

    let demo = demo_runner.inner_mut();

    // let sequencer_address = [1u8; 32].into();

    let value_setter_admin_private_key = DefaultPrivateKey::generate();
    let election_admin_private_key = DefaultPrivateKey::generate();

    let initial_balannce = 100000000;

    let genesis_config = create_demo_config(
        initial_balannce,
        &value_setter_admin_private_key,
        &election_admin_private_key,
    );
    demo.init_chain(genesis_config);

    demo.begin_slot(Default::default());
    let (prev_state_root, _, _) = demo.end_slot();
    let mut prev_state_root = prev_state_root.0;

    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::WARN)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|_err| eprintln!("Unable to set global default subscriber"))
        .expect("Cannot fail to set subscriber");




    for height in START_HEIGHT.. {
        println!("Requesting data for height {} and prev_state_root 0x{}", height, hex::encode(&prev_state_root));
        let filtered_block = da_service.get_finalized_at(height).await?;
    }

    // let cel_service = default_celestia_service();

    // let mut demo = DemoApp::new(
    //     storage.clone(),
    //     Runtime::new(),
    //     DemoAppTxVerifier::new(),
    //     DemoAppTxHooks::new(),
    //     GenesisConfig::new(
    //         (),
    //         bank::BankConfig {
    //             tokens: vec![TokenConfig {
    //                 token_name: "sovereign".to_string(),
    //                 address_and_balances: vec![(sequencer_address, 1000)],
    //             }],
    //         },
    //         accounts::AccountConfig { pub_keys: vec![] },
    //     ),
    // );
    //
    // let rpc_ledger = ledger_db.clone();
    // let rpc_storage = storage.clone();
    //
    // let _rpc_handle = rpc::RpcProvider::start(rpc_ledger, rpc_storage).await?;
    //
    // let mut item_numbers = ledger_db.get_next_items_numbers();
    // if item_numbers.slot_number == 1 {
    //     print!("No history detected. Initializing chain...");
    //     demo.init_chain(());
    //     println!("Done.");
    // } else {
    //     println!("Chain is already initialized. Skipping initialization.");
    // }
    // let last_slot_processed_before_shutdown = item_numbers.slot_number - 1;
    //
    // println!("Beginning sync from da slot {}...", START_HEIGHT);
    // for i in 0.. {
    //     let height = START_HEIGHT + i;
    //     if last_slot_processed_before_shutdown > i {
    //         continue;
    //     }
    //
    //     let filtered_block: FilteredCelestiaBlock = cel_service.get_finalized_at(height).await?;
    //     let slot_hash = filtered_block.hash();
    //     let mut data_to_persist = SlotCommitBuilder::default();
    //     let batches = cel_service.extract_relevant_txs(filtered_block);
    //
    //     demo.begin_slot();
    //     let num_batches = batches.len();
    //     for raw_batch in batches {
    //         let mut data = raw_batch.data();
    //         let batch = match Batch::decode(&mut data) {
    //             Ok(batch) => batch,
    //             Err(e) => {
    //                 println!("    Error decoding batch: {}. Skipping.", e);
    //                 continue;
    //             }
    //         };
    //         let batch_hash = sha2(batch.encode_to_vec());
    //         let tx_start = item_numbers.tx_number;
    //         let num_txs = batch.txs.len();
    //         let mut batch_to_store = StoredBatch {
    //             sender: raw_batch.sender.as_ref().to_vec(),
    //             hash: batch_hash,
    //             extra_data: DbBytes::new(raw_batch.sender.as_ref().to_vec()),
    //             txs: TxNumber(tx_start)..TxNumber(tx_start + num_txs as u64),
    //             status: Status::Skipped,
    //         };
    //         item_numbers.tx_number += num_txs as u64;
    //         print!("    Applying batch of {} transactions...", num_txs);
    //
    //         match demo.apply_batch(batch.clone(), raw_batch.sender().as_ref(), None) {
    //             Ok(events) => {
    //                 println!(" Done!");
    //                 batch_to_store.status = Status::Applied;
    //                 data_to_persist.txs.extend(batch.txs.into_iter().map(|tx| {
    //                     let start_event_number = item_numbers.event_number;
    //                     let end_event_number = start_event_number + events.len() as u64;
    //                     item_numbers.event_number = end_event_number;
    //                     StoredTransaction {
    //                         hash: sha2(&tx.data[..]),
    //                         events: EventNumber(start_event_number)..EventNumber(end_event_number),
    //                         data: DbBytes::new(tx.data),
    //                         status: Status::Applied,
    //                     }
    //                 }));
    //                 data_to_persist.batches.push(batch_to_store);
    //
    //                 data_to_persist.events.extend(events.into_iter());
    //             }
    //             Err(e) => {
    //                 println!(
    //                     " Uh-oh! Failed to apply batch. Applying consensus set update {:?}",
    //                     e
    //                 );
    //             }
    //         }
    //     }
    //
    //     demo.end_slot();
    //     data_to_persist.slot_data = Some(StoredSlot {
    //         hash: slot_hash,
    //         extra_data: DbBytes::new(vec![]),
    //         batches: BatchNumber(item_numbers.batch_number)
    //             ..BatchNumber(item_numbers.batch_number + num_batches as u64),
    //     });
    //     item_numbers.batch_number += num_batches as u64;
    //     ledger_db.commit_slot(data_to_persist.finalize()?)?;
    // }

    Ok(())
}


// pub fn default_celestia_service() -> CelestiaService {
//     let mut headers = HeaderMap::new();
//     // TODO: read from environment variable
//     // read string from environment variable CELESTIA_NODE_AUTH_TOKEN :
//     // let token = std::env::var("CELESTIA_NODE_AUTH_TOKEN").unwrap();
//     headers.insert(
//         "Authorization",
//         format!("Bearer {}", CELESTIA_NODE_AUTH_TOKEN)
//             .parse()
//             .unwrap(),
//     );
//     let client = jsonrpsee::http_client::HttpClientBuilder::default()
//         .set_headers(headers)
//         .max_request_body_size(1024 * 1024 * 100) // 100 MB
//         .build("http://localhost:11111/")
//         .unwrap();
//     CelestiaService::with_client(client)
// }
//
// fn sha2(data: impl AsRef<[u8]>) -> [u8; 32] {
//     let mut hasher = Sha256::new();
//     hasher.update(data.as_ref());
//     hasher.finalize().into()
// }
//
