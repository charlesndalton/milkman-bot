use ethers::abi::Token;
use ethers::prelude::*;
use hex::FromHex;

use crate::constants::{APP_DATA, ERC20_BALANCE, KIND_SELL};

#[derive(Debug)]
pub struct SignatureData<'a> {
    pub from_token: Address,
    pub to_token: Address,
    pub receiver: Address,
    pub sell_amount_after_fees: U256,
    pub buy_amount_after_fees_and_slippage: U256,
    pub valid_to: u64,
    pub fee_amount: U256,
    pub order_creator: Address,
    pub price_checker: Address,
    pub price_checker_data: &'a Bytes,
}

pub fn get_eip_1271_signature(
    SignatureData {
        from_token,
        to_token,
        receiver,
        sell_amount_after_fees,
        buy_amount_after_fees_and_slippage,
        valid_to,
        fee_amount,
        order_creator,
        price_checker,
        price_checker_data,
    }: SignatureData<'_>,
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
