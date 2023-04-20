use std::net::SocketAddr;

use bank::{
    query::{BankRpcImpl, BankRpcServer},
    Bank,
};
use jsonrpsee::server::ServerHandle;
use sov_modules_api::mocks::MockContext;
use sov_state::{mocks::MockStorageSpec, ProverStorage, WorkingSet};
use sovereign_db::ledger_db::LedgerDB;
use sovereign_sdk::rpc::{LedgerRpcProvider, QueryMode, SlotIdentifier};

use crate::{runtime::Runtime, tx_verifier_impl::DemoAppTxVerifier, Spec};

#[derive(Clone)]
pub(crate) struct RpcProvider {
    pub ledger_db: LedgerDB<Spec>,
    pub state_db: ProverStorage<MockStorageSpec>,
    pub runtime: Runtime<MockContext>,
    pub _tx_verifier: DemoAppTxVerifier<MockContext>,
}

impl BankRpcImpl<MockContext> for RpcProvider {
    fn get_backing_impl(&self) -> &Bank<MockContext> {
        &self.runtime.bank
    }

    fn get_working_set(&self) -> WorkingSet<ProverStorage<MockStorageSpec>> {
        WorkingSet::new(self.state_db.clone())
    }
}

impl RpcProvider {
    pub async fn start(
        ledger_db: LedgerDB<Spec>,
        state_db: ProverStorage<MockStorageSpec>,
    ) -> Result<ServerHandle, jsonrpsee::core::Error> {
        let address: SocketAddr = "127.0.0.1:12345".parse().unwrap();
        let server = jsonrpsee::server::ServerBuilder::default()
            .build([address].as_ref())
            .await?;

        let runtime = Runtime::new();
        let tx_verifier = DemoAppTxVerifier::new();
        let bank_rpc = Self {
            ledger_db,
            state_db,
            runtime,
            _tx_verifier: tx_verifier,
        };

        let ledger_rpc = bank_rpc.clone();

        let mut bank_rpc = bank_rpc.into_rpc();
        bank_rpc.register_method("chain_getSlots", move |params, _| {
            let slot_ids: Vec<SlotIdentifier>;
            let query_mode: QueryMode;
            (slot_ids, query_mode) = params.parse()?;
            ledger_rpc
                .get_slots(&slot_ids, query_mode)
                .map_err(|e| e.into())
        })?;

        server.start(bank_rpc)
    }
}

// TODO: Re-implement
// impl TransactionRpcProvider for RpcProvider {
//     type Transaction = RawTx;

//     fn check_transaction(&self, tx: Self::Transaction) -> Result<(), anyhow::Error> {
//         self.tx_verifier.verify_tx_stateless(tx).map(|_| ())
//     }

//     fn submit_transaction(&self, tx: Self::Transaction) -> Result<(), anyhow::Error> {
//         todo!()
//     }
// }

/// Delegate all the Ledger RPC methods to the LedgerDB.
impl LedgerRpcProvider for RpcProvider {
    type SlotResponse = <LedgerDB<Spec> as LedgerRpcProvider>::SlotResponse;

    type BatchResponse = <LedgerDB<Spec> as LedgerRpcProvider>::BatchResponse;

    type TxResponse = <LedgerDB<Spec> as LedgerRpcProvider>::TxResponse;

    type EventResponse = <LedgerDB<Spec> as LedgerRpcProvider>::EventResponse;

    fn get_slots(
        &self,
        slot_ids: &[sovereign_sdk::rpc::SlotIdentifier],
        query_mode: sovereign_sdk::rpc::QueryMode,
    ) -> Result<Vec<Option<Self::SlotResponse>>, anyhow::Error> {
        self.ledger_db.get_slots(slot_ids, query_mode)
    }

    fn get_batches(
        &self,
        batch_ids: &[sovereign_sdk::rpc::BatchIdentifier],
        query_mode: sovereign_sdk::rpc::QueryMode,
    ) -> Result<Vec<Option<Self::BatchResponse>>, anyhow::Error> {
        self.ledger_db.get_batches(batch_ids, query_mode)
    }

    fn get_transactions(
        &self,
        tx_ids: &[sovereign_sdk::rpc::TxIdentifier],
        query_mode: sovereign_sdk::rpc::QueryMode,
    ) -> Result<Vec<Option<Self::TxResponse>>, anyhow::Error> {
        self.ledger_db.get_transactions(tx_ids, query_mode)
    }

    fn get_events(
        &self,
        event_ids: &[sovereign_sdk::rpc::EventIdentifier],
    ) -> Result<Option<Vec<Self::EventResponse>>, anyhow::Error> {
        self.ledger_db.get_events(event_ids)
    }
}
