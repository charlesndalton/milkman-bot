use ethers::abi::Address;
use ethers::types::{Bytes, U256};

#[derive(Debug, PartialEq, Clone)]
pub struct Swap {
    pub order_contract: Address, // 1 swap per contract so this can be used as a UID
    pub order_creator: Address,
    pub receiver: Address,
    pub from_token: Address,
    pub to_token: Address,
    pub amount_in: U256,
    pub price_checker: Address,
    pub price_checker_data: Bytes,
}

pub type BlockNumber = u64;
