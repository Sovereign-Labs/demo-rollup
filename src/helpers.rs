use sov_modules_api::{mocks::MockContext, DispatchQuery};
use sov_state::{mocks::MockStorageSpec, ProverStorage, WorkingSet};

use crate::runtime::Runtime;

pub(crate) fn run_query(query: Vec<u8>, storage: ProverStorage<MockStorageSpec>) -> String {
    let module = Runtime::<MockContext>::decode_query(&query).unwrap();
    let query_response = module.dispatch_query(&mut WorkingSet::new(storage));

    String::from_utf8(query_response.response).unwrap()
}
