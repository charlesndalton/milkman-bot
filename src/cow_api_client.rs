use crate::MILKMAN_ADDRESS;
use anyhow::Result;
use ethers::abi::Address;
use ethers::types::U256;
use log::info;
use serde_json::Value;

#[derive(Debug)]
pub struct FeeAndQuote {
    pub fee_amount: U256,
    pub buy_amount_after_fee: U256,
}

pub async fn get_fee_and_quote(
    sell_token: Address,
    buy_token: Address,
    sell_amount_before_fee: U256,
) -> Result<FeeAndQuote> {
    let client = reqwest::Client::new();

    let response = client
        .get("https://api.cow.fi/mainnet/api/v1/feeAndQuote/sell")
        .query(&[("sellToken", address_to_string(sell_token))])
        .query(&[("buyToken", address_to_string(buy_token))])
        .query(&[("sellAmountBeforeFee", sell_amount_before_fee.as_u128())])
        .send()
        .await?
        .error_for_status()?;

    let response_body = response.json::<Value>().await?;

    println!("{:?}", response_body);

    let fee_amount = response_body["fee"]["amount"].as_str().unwrap().to_owned();
    let buy_amount_after_fee = response_body["buyAmountAfterFee"]
        .as_str()
        .unwrap()
        .to_owned();

    Ok(FeeAndQuote {
        fee_amount: fee_amount.parse::<u128>()?.into(),
        buy_amount_after_fee: buy_amount_after_fee.parse::<u128>()?.into(),
    })
}

pub async fn create_order(
    sell_token: Address,
    buy_token: Address,
    sell_amount: U256,
    buy_amount: U256,
    valid_to: u64,
    fee_amount: U256,
    receiver: Address,
) -> Result<String> {
    let client = reqwest::Client::new();
    let order_uid = client
        .post("https://api.cow.fi/mainnet/api/v1/orders")
        .json(&serde_json::json!({
            "sellToken": address_to_string(sell_token),
            "buyToken": address_to_string(buy_token),
            "sellAmount": sell_amount.to_string(),
            "buyAmount": buy_amount.to_string(),
            "validTo": valid_to,
            "appData": "0x2B8694ED30082129598720860E8E972F07AA10D9B81CAE16CA0E2CFB24743E24",
            "feeAmount": fee_amount.to_string(),
            "kind": "sell",
            "partiallyFillable": false,
            "receiver": address_to_string(receiver),
            "signature": MILKMAN_ADDRESS,
            "from": MILKMAN_ADDRESS,
            "sellTokenBalance": "erc20",
            "buyTokenBalance": "erc20",
            "signingScheme": "presign"
        }))
        .send()
        .await?
        .json::<Value>()
        .await?;

    info!("created order with UID {:?}", order_uid);

    Ok(order_uid.as_str().unwrap().to_owned())
}

fn address_to_string(address: Address) -> String {
    "0x".to_owned() + &hex::encode(address.to_fixed_bytes())
}
