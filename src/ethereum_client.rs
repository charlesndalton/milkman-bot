use crate::types::{BlockNumber, Swap};
use anyhow::{anyhow, Result};
use ethers::prelude::*;
use hex::FromHex;
use log::debug;
#[cfg(test)]
use rand::prelude::*;
use std::convert::{From, Into};
use std::sync::Arc;

use crate::configuration::Configuration;
use crate::constants::{APP_DATA, ERC20_BALANCE, KIND_SELL};
use crate::encoder;

abigen!(
    RawMilkman,
    "./abis/Milkman.json",
    event_derives(serde::Deserialize, serde::Serialize),
);

abigen!(
    RawHashHelper,
    "./abis/HashHelper.json",
    event_derives(serde::Deserialize, serde::Serialize),
);

abigen!(
    RawERC20,
    "./abis/ERC20.json",
    event_derives(serde::Deserialize, serde::Serialize),
);

pub type Milkman = RawMilkman<Provider<Http>>;
pub type HashHelper = RawHashHelper<Provider<Http>>;
pub type ERC20 = RawERC20<Provider<Http>>;

pub struct EthereumClient {
    inner_client: Arc<Provider<Http>>,
    milkman: Milkman,
}

impl EthereumClient {
    pub fn new(config: &Configuration) -> Result<Self> {
        let node_url = if config.node_base_url.is_some() {
            config.node_base_url.clone().unwrap()
        } else {
            format!(
                "https://{}.infura.io/v3/{}",
                config.network,
                config.infura_api_key.clone().unwrap()
            )
        };
        let provider = Arc::new(Provider::<Http>::try_from(node_url)?);

        Ok(Self {
            milkman: Milkman::new(config.milkman_address, Arc::clone(&provider)),
            inner_client: provider,
        })
    }

    pub async fn get_latest_block_number(&self) -> Result<u64> {
        self.get_latest_block()
            .await?
            .number
            .ok_or(anyhow!("Error extracting number from latest block."))
            .map(|block_num: U64| block_num.try_into().unwrap()) // U64 -> u64 should always work
    }

    #[allow(dead_code)]
    pub async fn get_chain_timestamp(&self) -> Result<u64> {
        Ok(self.get_latest_block().await?.timestamp.as_u64())
    }

    async fn get_latest_block(&self) -> Result<Block<H256>> {
        self.inner_client
            .get_block(ethers::core::types::BlockNumber::Latest)
            .await?
            .ok_or(anyhow!("Error fetching latest block."))
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

    pub async fn get_balance_of(&self, token_address: Address, user: Address) -> Result<U256> {
        let token = ERC20::new(token_address, Arc::clone(&self.inner_client));

        Ok(token.balance_of(user).call().await?)
    }

    /// To estimate the amount of gas it'll take to call `isValidSignature`, we
    /// create a mock order & signature based on the existing order and use those
    /// along with ethers-rs's `estimate_gas()`.
    pub async fn get_estimated_order_contract_gas(
        &self,
        config: &Configuration,
        swap_request: &Swap,
    ) -> Result<U256> {
        let order_contract =
            Milkman::new(swap_request.order_contract, Arc::clone(&self.inner_client));

        let hash_helper =
            HashHelper::new(config.hash_helper_address, Arc::clone(&self.inner_client));

        let domain_separator = self.milkman.domain_separator().call().await?;

        let mock_order = Data {
            sell_token: swap_request.from_token,
            buy_token: swap_request.to_token,
            receiver: swap_request.receiver,
            sell_amount: swap_request.amount_in,
            buy_amount: U256::MAX,
            valid_to: u32::MAX,
            app_data: Vec::from_hex(APP_DATA).unwrap().try_into().unwrap(),
            fee_amount: U256::zero(),
            kind: Vec::from_hex(KIND_SELL).unwrap().try_into().unwrap(),
            partially_fillable: false,
            sell_token_balance: Vec::from_hex(ERC20_BALANCE).unwrap().try_into().unwrap(),
            buy_token_balance: Vec::from_hex(ERC20_BALANCE).unwrap().try_into().unwrap(),
        };

        let mock_order_digest = hash_helper
            .hash(mock_order, domain_separator)
            .call()
            .await?;

        let mock_signature = encoder::get_eip_1271_signature(
            swap_request.from_token,
            swap_request.to_token,
            swap_request.receiver,
            swap_request.amount_in,
            U256::MAX,
            u32::MAX as u64,
            U256::zero(),
            swap_request.order_creator,
            swap_request.price_checker,
            &swap_request.price_checker_data,
        );

        debug!(
            "Is valid sig? {:?}",
            order_contract
                .is_valid_signature(mock_order_digest, mock_signature.clone())
                .call()
                .await?
        );

        Ok(order_contract
            .is_valid_signature(mock_order_digest, mock_signature)
            .estimate_gas()
            .await?)
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

    #[tokio::test]
    async fn test_ethereum_client() {
        let config = Configuration {
            infura_api_key: Some("e74132f416d346308763252779d7df22".to_string()),
            network: "goerli".to_string(),
            milkman_address: "0x5D9C7CBeF995ef16416D963EaCEEC8FcA2590731"
                .parse()
                .unwrap(),
            hash_helper_address: "0x429A101f42781C53c088392956c95F0A32437b8C"
                .parse()
                .unwrap(),
            starting_block_number: None,
            polling_frequency_secs: 15,
            node_base_url: None,
        };

        let eth_client = EthereumClient::new(&config).expect("Unable to create Ethereum client");

        let latest_block_num = eth_client
            .get_latest_block_number()
            .await
            .expect("Unable to get latest block number");

        assert!(latest_block_num > 7994445);

        let chain_timestamp = eth_client
            .get_chain_timestamp()
            .await
            .expect("Unable to get chain timestamp");

        assert!(chain_timestamp > 1669053987);

        let goerli_uni_addr = "0x1f9840a85d5aF5bf1D1762F925BDADdC4201F984"
            .parse()
            .unwrap();
        let goerli_uni_whale = "0x41653c7d61609D856f29355E404F310Ec4142Cfb"
            .parse()
            .unwrap();

        let balance = eth_client
            .get_balance_of(goerli_uni_addr, goerli_uni_whale)
            .await
            .expect("Unable to get balance");

        assert!(balance > 0.into());

        let requested_swaps = eth_client
            .get_requested_swaps(0, latest_block_num)
            .await
            .expect("Unable to get requested swaps");

        assert!(!requested_swaps.is_empty());
    }

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
