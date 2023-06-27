use crate::configuration::Configuration;
use anyhow::{anyhow, Context, Result};
use ethers::abi::Address;
use ethers::types::{Bytes, U256};
use log::{debug, info};
use serde_json::Value;

use crate::constants::APP_DATA;

#[derive(Debug)]
pub struct Quote {
    pub fee_amount: U256,
    pub buy_amount_after_fee: U256,
    pub valid_to: u64,
    pub id: u64,
}

#[derive(Debug)]
pub struct Order<'a> {
    pub order_contract: Address,
    pub sell_token: Address,
    pub buy_token: Address,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub valid_to: u64,
    pub fee_amount: U256,
    pub receiver: Address,
    pub eip_1271_signature: &'a Bytes,
    pub quote_id: u64,
}

pub struct CowAPIClient {
    pub base_url: String,
    pub milkman_address: String,
}

impl CowAPIClient {
    pub fn new(config: &Configuration) -> Self {
        Self {
            base_url: format!("https://api.cow.fi/{}/api/v1/", config.network),
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
                "sellToken": sell_token,
                "buyToken": buy_token,
                "sellAmountBeforeFee": sell_amount_before_fee.to_string(),
                "appData": "0x".to_string() + APP_DATA,
                "kind": "sell",
                "partiallyFillable": false,
                "from": order_contract,
                "sellTokenBalance": "erc20",
                "buyTokenBalance": "erc20",
                "signingScheme": "eip1271",
                "onchainOrder": true,
                "priceQuality": "optimal",
                "verificationGasLimit": verification_gas_limit,
            }))
            .send()
            .await?;

        let response_body = match response.error_for_status_ref() {
            Ok(_) => response.json::<Value>().await?,
            Err(err) => {
                debug!("GET quote failed with body: {:?}", response.text().await?);
                return Err(anyhow!(err));
            }
        };

        debug!(
            "Got back the following response body from the quote endpoint: {:?}",
            response_body
        );

        let quote = &response_body["quote"];
        let fee_amount = quote["feeAmount"]
            .as_str()
            .context("unable to get `feeAmount` on quote")?
            .to_owned();
        let buy_amount_after_fee = quote["buyAmount"]
            .as_str()
            .context("unable to get `buyAmountAfterFee` from quote")?
            .to_owned();
        let valid_to = quote["validTo"]
            .as_u64()
            .context("unable to get `validTo` from quote")?;
        let id = response_body["id"]
            .as_u64()
            .context("unable to get `id` from quote")?;

        Ok(Quote {
            fee_amount: fee_amount.parse::<u128>()?.into(),
            buy_amount_after_fee: buy_amount_after_fee.parse::<u128>()?.into(),
            valid_to,
            id,
        })
    }

    pub async fn create_order(
        &self,
        Order {
            order_contract,
            sell_token,
            buy_token,
            sell_amount,
            buy_amount,
            valid_to,
            fee_amount,
            receiver,
            eip_1271_signature,
            quote_id,
        }: Order<'_>,
    ) -> Result<String> {
        let http_client = reqwest::Client::new();
        let response = http_client
            .post(self.base_url.clone() + "orders")
            .json(&serde_json::json!({
                "sellToken": sell_token,
                "buyToken": buy_token,
                "sellAmount": sell_amount.to_string(),
                "buyAmount": buy_amount.to_string(),
                "validTo": valid_to,
                "appData": "0x2B8694ED30082129598720860E8E972F07AA10D9B81CAE16CA0E2CFB24743E24",
                "feeAmount": fee_amount.to_string(),
                "kind": "sell",
                "partiallyFillable": false,
                "receiver": receiver,
                "signature": eip_1271_signature.to_string(),
                "from": order_contract,
                "sellTokenBalance": "erc20",
                "buyTokenBalance": "erc20",
                "signingScheme": "eip1271",
                "quoteId": quote_id,
            }))
            .send()
            .await?;

        let order_uid = match response.error_for_status_ref() {
            Ok(_) => response
                .json::<Value>()
                .await?
                .as_str()
                .context("Unable to retrieve UID from POST order response")?
                .to_string(),
            Err(err) => {
                debug!("POST order failed with body: {:?}", response.text().await?);
                return Err(anyhow!(err));
            }
        };

        info!("created order with UID {}", order_uid);

        Ok(order_uid)
    }
}
