use ethers::abi::Address;
use ethers::types::U256;

pub struct Swap {
    pub swap_id: [u8; 32], // bytes32
    pub user: Address,
    pub receiver: Address,
    pub from_token: Address,
    pub to_token: Address,
    pub amount_in: U256,
    pub price_checker: Address,
    pub nonce: U256,
}

pub type BlockNumber = u64;
