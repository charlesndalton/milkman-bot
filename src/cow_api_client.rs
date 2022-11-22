use crate::configuration::Configuration;
use anyhow::{anyhow, Result};
use ethers::abi::Address;
use ethers::types::{Bytes, U256};
use log::info;
use serde_json::Value;

use crate::constants::{APP_DATA};

#[derive(Debug)]
pub struct Quote {
    pub fee_amount: U256,
    pub buy_amount_after_fee: U256,
    pub valid_to: u64,
}

pub struct CowAPIClient {
    pub base_url: String,
    pub milkman_address: String,
}

impl CowAPIClient {
    pub fn new(config: &Configuration) -> Self {
        Self {
            base_url: format!("https://barn.api.cow.fi/{}/api/v1/", config.network),
            milkman_address: config.milkman_address.to_string(),
        }
    }

    pub async fn get_quote(
        &self,
        order_contract: Address,
        sell_token: Address,
        buy_token: Address,
        sell_amount_before_fee: U256,
        verification_gas_limit: u64,
    ) -> Result<Quote> {
        let http_client = reqwest::Client::new();

        let response = http_client
            .post(self.base_url.clone() + "quote")
            .json(&serde_json::json!({
                "sellToken": address_to_string(sell_token),
                "buyToken": address_to_string(buy_token),
                "sellAmountBeforeFee": sell_amount_before_fee.to_string(),
                "appData": "0x".to_string() + APP_DATA,
                "kind": "sell",
                "partiallyFillable": false,
                "from": address_to_string(order_contract),
                "sellTokenBalance": "erc20",
                "buyTokenBalance": "erc20",
                "signingScheme": "eip1271",
                "onchainOrder": true,
                "priceQuality": "optimal",
                "verificationGasLimit": verification_gas_limit,
            }))
            .send()
            .await?
            .error_for_status()?;

        let response_body = response.json::<Value>().await?;

        println!("{:?}", response_body);

        let quote = &response_body["quote"];
        let fee_amount = quote["feeAmount"].as_str().ok_or(anyhow!("unable to get `feeAmount` on quote"))?.to_owned();
        let buy_amount_after_fee = quote["buyAmount"].as_str().ok_or(anyhow!("unable to get `buyAmountAfterFee` from quote"))?.to_owned();
        let valid_to = quote["validTo"].as_u64().ok_or(anyhow!("unable to get `validTo` from quote"))?;

        Ok(Quote {
            fee_amount: fee_amount.parse::<u128>()?.into(),
            buy_amount_after_fee: buy_amount_after_fee.parse::<u128>()?.into(),
            valid_to
        })
    }

    pub async fn create_order(
        &self,
        order_contract: Address,
        sell_token: Address,
        buy_token: Address,
        sell_amount: U256,
        buy_amount: U256,
        valid_to: u64,
        fee_amount: U256,
        receiver: Address,
        eip_1271_signature: &Bytes,
    ) -> Result<String> {
        let http_client = reqwest::Client::new();
        let response = http_client
            .post(self.base_url.clone() + "orders")
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
                "signature": eip_1271_signature.to_string(),
                "from": address_to_string(order_contract),
                "sellTokenBalance": "erc20",
                "buyTokenBalance": "erc20",
                "signingScheme": "eip1271"
            }))
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;

        match response.as_str() {
            Some(order_uid) => {
                info!("created order with UID {:?}", order_uid);
                Ok(order_uid.to_owned())
            }
            None => Err(anyhow!("Unable to retrieve UID from order generation")),
        }
    }
}

fn address_to_string(address: Address) -> String {
    "0x".to_owned() + &hex::encode(address.to_fixed_bytes())
}
