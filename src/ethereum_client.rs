use crate::types::{BlockNumber, Swap};
use crate::MILKMAN_ADDRESS;
use anyhow::{anyhow, Result};
use ethers::prelude::*;
use std::convert::{From, Into};
use std::sync::Arc;

abigen!(
    RawMilkman,
    "./abis/Milkman.json",
    event_derives(serde::Deserialize, serde::Serialize),
);

pub type Milkman = RawMilkman<Provider<Http>>;

pub struct EthereumClient {
    inner_client: Provider<Http>,
    milkman: Milkman,
}

impl EthereumClient {
    pub fn new(infura_api_key: &str) -> Result<Self> {
        let infura_url = format!("https://mainnet.infura.io/v3/{}", infura_api_key);
        let provider = Provider::<Http>::try_from(infura_url)?;

        Ok(Self {
            inner_client: provider,
            milkman: Milkman::new(MILKMAN_ADDRESS.parse::<Address>()?, Arc::new(provider)),
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
            .from_block(to_block)
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
            swap_id: raw_swap_request.swap_id,
            user: raw_swap_request.user,
            receiver: raw_swap_request.receiver,
            from_token: raw_swap_request.from_token,
            to_token: raw_swap_request.to_token,
            amount_in: raw_swap_request.amount_in,
            price_checker: raw_swap_request.price_checker,
            nonce: raw_swap_request.nonce,
        }
    }
}
