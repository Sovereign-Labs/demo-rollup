use anyhow::bail;
use sov_modules_api::{
    mocks::{MockPublicKey, MockSignature},
    AddressTrait, Context, Spec,
};
use sov_state::{mocks::MockStorageSpec, ProverStorage};
use sovereign_sdk::core::types::ArrayWitness;

/// Context for the demo rollup.
#[derive(Clone, Debug, PartialEq)]
pub struct DemoContext {
    pub sender: Address,
}

impl Spec for DemoContext {
    type Address = Address;
    type Storage = ProverStorage<MockStorageSpec>;
    type Hasher = sha2::Sha256;
    type PublicKey = MockPublicKey;
    type Signature = MockSignature;
    type Witness = ArrayWitness;
}

impl Context for DemoContext {
    fn sender(&self) -> &Self::Address {
        &self.sender
    }

    fn new(sender: Self::Address) -> Self {
        Self { sender }
    }
}

impl AddressTrait for Address {}

/// Default implementation of AddressTrait for the module system
#[derive(borsh::BorshDeserialize, borsh::BorshSerialize, Debug, PartialEq, Clone, Eq)]
pub struct Address {
    addr: [u8; 32],
}

impl<'a> TryFrom<&'a [u8]> for Address {
    type Error = anyhow::Error;

    fn try_from(addr: &'a [u8]) -> Result<Self, Self::Error> {
        if addr.len() != 32 {
            bail!("Address must be 32 bytes long");
        }
        let mut addr_bytes = [0u8; 32];
        addr_bytes.copy_from_slice(&addr);
        Ok(Self { addr: addr_bytes })
    }
}

impl AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        self.addr.as_ref()
    }
}
