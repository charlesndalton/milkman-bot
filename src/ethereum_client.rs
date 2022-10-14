use crate::types::{BlockNumber, Swap};
use anyhow::{anyhow, Result};
use ethers::prelude::*;
#[cfg(test)]
use rand::prelude::*;
use std::convert::{From, Into};
use std::sync::Arc;

use crate::configuration::Configuration;

abigen!(
    RawMilkman,
    "./abis/Milkman.json",
    event_derives(serde::Deserialize, serde::Serialize),
);

pub type Milkman = RawMilkman<Provider<Http>>;

pub struct EthereumClient {
    inner_client: Arc<Provider<Http>>,
    milkman: Milkman,
}

impl EthereumClient {
    pub fn new(config: &Configuration) -> Result<Self> {
        let infura_url = format!(
            "https://{}.infura.io/v3/{}",
            config.network, config.infura_api_key
        );
        let provider = Arc::new(Provider::<Http>::try_from(infura_url)?);

        Ok(Self {
            milkman: Milkman::new(
                config.milkman_address.parse::<Address>()?,
                Arc::clone(&provider),
            ),
            inner_client: provider,
        })
    }

    pub async fn get_latest_block_number(&self) -> Result<u64> {
        self.inner_client
            .get_block(ethers::core::types::BlockNumber::Latest)
            .await?
            .ok_or(anyhow!("Error fetching latest block."))?
            .number
            .ok_or(anyhow!("Error extracting number from latest block."))
            .map(|block_num: U64| block_num.try_into().unwrap()) // U64 -> u64 should always work
    }

    pub async fn get_requested_swaps(
        &self,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> Result<Vec<Swap>> {
        Ok(self
            .milkman
            .swap_requested_filter()
            .from_block(from_block)
            .to_block(to_block)
            .query()
            .await?
            .iter()
            .map(Into::into)
            .collect())
    }
}

impl From<&SwapRequestedFilter> for Swap {
    fn from(raw_swap_request: &SwapRequestedFilter) -> Self {
        Self {
            order_contract: raw_swap_request.order_contract,
            order_creator: raw_swap_request.order_creator,
            receiver: raw_swap_request.to,
            from_token: raw_swap_request.from_token,
            to_token: raw_swap_request.to_token,
            amount_in: raw_swap_request.amount_in,
            price_checker: raw_swap_request.price_checker,
            price_checker_data: raw_swap_request.price_checker_data.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_swap() {
        let order_contract = Address::random();
        let order_creator = Address::random();
        let amount_in: U256 = rand::thread_rng().gen::<u128>().into();
        let from_token = Address::random();
        let to_token = Address::random();
        let to = Address::random();
        let price_checker = Address::random();
        let price_checker_data: Bytes = rand::thread_rng().gen::<[u8; 1000]>().into();

        let raw_swap = SwapRequestedFilter {
            order_contract,
            order_creator,
            amount_in,
            from_token,
            to_token,
            to,
            price_checker,
            price_checker_data: price_checker_data.clone(),
        };
        let converted: Swap = (&raw_swap).into();

        assert_eq!(converted.order_contract, order_contract);
        assert_eq!(converted.order_creator, order_creator);
        assert_eq!(converted.amount_in, amount_in);
        assert_eq!(converted.from_token, from_token);
        assert_eq!(converted.to_token, to_token);
        assert_eq!(converted.receiver, to);
        assert_eq!(converted.price_checker, price_checker);
        assert_eq!(converted.price_checker_data, price_checker_data);
    }
}
