use ethers::abi::Token;
use ethers::prelude::*;
use hex::FromHex;

use crate::constants::{APP_DATA, ERC20_BALANCE, KIND_SELL};

#[allow(clippy::too_many_arguments)]
pub fn get_eip_1271_signature(
    from_token: Address,
    to_token: Address,
    receiver: Address,
    sell_amount_after_fees: U256,
    buy_amount_after_fees_and_slippage: U256,
    valid_to: u64,
    fee_amount: U256,
    order_creator: Address,
    price_checker: Address,
    price_checker_data: &Bytes,
) -> Bytes {
    abi::encode(&vec![
        Token::Address(from_token),
        Token::Address(to_token),
        Token::Address(receiver),
        Token::Uint(sell_amount_after_fees),
        Token::Uint(buy_amount_after_fees_and_slippage),
        Token::Uint(valid_to.into()),
        Token::FixedBytes(Vec::from_hex(APP_DATA).unwrap()),
        Token::Uint(fee_amount),
        Token::FixedBytes(Vec::from_hex(KIND_SELL).unwrap()),
        Token::Bool(false), // partiallyFillable = false; this is fill or kill order
        Token::FixedBytes(Vec::from_hex(ERC20_BALANCE).unwrap()),
        Token::FixedBytes(Vec::from_hex(ERC20_BALANCE).unwrap()),
        Token::Address(order_creator),
        Token::Address(price_checker),
        Token::Bytes(price_checker_data.to_vec()),
    ])
    .into()
}
