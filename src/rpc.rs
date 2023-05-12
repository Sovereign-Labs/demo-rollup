use std::net::SocketAddr;

use bank::{
    query::{BankRpcImpl, BankRpcServer},
    Bank,
};
use jsonrpsee::{server::ServerHandle, RpcModule};
use sov_modules_api::mocks::MockContext;
use sov_state::{mocks::MockStorageSpec, ProverStorage, WorkingSet};
use sovereign_db::ledger_db::LedgerDB;
use sovereign_sdk::rpc::{
    BatchIdentifier, EventIdentifier, LedgerRpcProvider, QueryMode, SlotIdentifier, TxIdentifier,
};

use crate::{runtime::Runtime, tx_verifier_impl::DemoAppTxVerifier};

#[derive(Clone)]
pub(crate) struct RpcProvider {
    pub ledger_db: LedgerDB,
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
        ledger_db: LedgerDB,
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

        let mut bank_rpc = bank_rpc.into_rpc();

        register_ledger_rpc(&mut bank_rpc)?;

        server.start(bank_rpc)
    }
}

fn register_ledger_rpc(rpc: &mut RpcModule<RpcProvider>) -> Result<(), jsonrpsee::core::Error> {
    rpc.register_method("ledger_head", move |_, db| {
        db.get_head().map_err(|e| e.into())
    })?;

    rpc.register_method("ledger_getSlots", move |params, db| {
        let ids: Vec<SlotIdentifier>;
        let query_mode: QueryMode;
        (ids, query_mode) = params.parse()?;
        db.get_slots(&ids, query_mode).map_err(|e| e.into())
    })?;

    rpc.register_method("ledger_getBatches", move |params, db| {
        let ids: Vec<BatchIdentifier>;
        let query_mode: QueryMode;
        (ids, query_mode) = params.parse()?;
        db.get_batches(&ids, query_mode).map_err(|e| e.into())
    })?;

    rpc.register_method("ledger_getTransactions", move |params, db| {
        let ids: Vec<TxIdentifier>;
        let query_mode: QueryMode;
        (ids, query_mode) = params.parse()?;
        db.get_transactions(&ids, query_mode).map_err(|e| e.into())
    })?;

    rpc.register_method("ledger_getEvents", move |params, db| {
        let ids: Vec<EventIdentifier> = params.parse()?;
        db.get_events(&ids).map_err(|e| e.into())
    })?;

    Ok(())
}

// TODO: implement TransactionRpcProvider and expose an endpoint for it

/// Delegate all the Ledger RPC methods to the LedgerDB.
impl LedgerRpcProvider for RpcProvider {
    type SlotResponse = <LedgerDB as LedgerRpcProvider>::SlotResponse;

    type BatchResponse = <LedgerDB as LedgerRpcProvider>::BatchResponse;

    type TxResponse = <LedgerDB as LedgerRpcProvider>::TxResponse;

    type EventResponse = <LedgerDB as LedgerRpcProvider>::EventResponse;

    fn get_head(&self) -> Result<Option<Self::SlotResponse>, anyhow::Error> {
        self.ledger_db.get_head()
    }

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
    ) -> Result<Vec<Option<Self::EventResponse>>, anyhow::Error> {
        self.ledger_db.get_events(event_ids)
    }
}
