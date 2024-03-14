use anyhow::{anyhow, Context, Result};
use ethers::types::Address;
use log::debug;
use std::env;

use crate::constants::{MAINNET_HASH_HELPER_ADDRESS, PROD_MILKMAN_ADDRESS};

#[derive(Debug, Clone, PartialEq)]
pub struct Configuration {
    pub infura_api_key: Option<String>,
    pub network: String, // whatever infura accepts as a network e.g., 'mainnet' or 'goerli'
    pub milkman_address: Address,
    pub hash_helper_address: Address,
    pub starting_block_number: Option<u64>,
    pub polling_frequency_secs: u64,
    pub node_base_url: Option<String>,
}

impl Configuration {
    pub fn get_from_environment() -> Result<Self> {
        let infura_api_key = collect_optional_environment_variable("INFURA_API_KEY")?;
        let node_base_url = collect_optional_environment_variable("NODE_BASE_URL")?;

        if infura_api_key.is_none() && node_base_url.is_none() {
            return Err(anyhow!(
                "either `infura_api_key` or `node_base_url` must be set"
            ));
        }

        let network = collect_optional_environment_variable("MILKMAN_NETWORK")?
            .unwrap_or("mainnet".to_string());
        let milkman_address = collect_optional_environment_variable("MILKMAN_ADDRESS")?
            .unwrap_or(PROD_MILKMAN_ADDRESS.to_string())
            .parse()?;
        let polling_frequency_secs =
            collect_optional_environment_variable("POLLING_FREQUENCY_SECS")?
                .map(|var| var.parse::<u64>())
                .transpose()?
                .unwrap_or(10);
        let hash_helper_address = collect_optional_environment_variable("HASH_HELPER_ADDRESS")?
            .unwrap_or(MAINNET_HASH_HELPER_ADDRESS.to_string())
            .parse()?;

        let starting_block_number =
            match collect_optional_environment_variable("STARTING_BLOCK_NUMBER")? {
                Some(block_num) => block_num.parse::<u64>().ok(),
                None => None,
            };

        Ok(Self {
            infura_api_key,
            network,
            milkman_address,
            hash_helper_address,
            starting_block_number,
            polling_frequency_secs,
            node_base_url,
        })
    }
}

#[allow(dead_code)]
fn collect_required_environment_variable(key: &str) -> Result<String> {
    env::var(key).context(format!("required environment variable {} not set", key))
}

fn collect_optional_environment_variable(key: &str) -> Result<Option<String>> {
    match env::var(key) {
        Ok(value) => Ok(Some(value)),
        Err(_) => {
            debug!(
                "environment variable {} not set but it wasn't required",
                key
            );
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_config() {
        setup_env_vars(Some("a"), Some("c"));
        let config = Configuration::get_from_environment().expect("failed to get");
        check_configuration(config, Some("a"), None);

        setup_env_vars(Some("a"), Some("123"));
        let config = Configuration::get_from_environment().expect("failed to get");
        check_configuration(config, Some("a"), Some(123));

        setup_env_vars(None, Some("123"));
        assert!(Configuration::get_from_environment().is_err());

        setup_env_vars(Some("a"), None);
        let config = Configuration::get_from_environment().expect("failed to get");
        check_configuration(config, Some("a"), None);
    }

    fn setup_env_vars(infura_api_key: Option<&str>, starting_block_number: Option<&str>) {
        fn setup_env_var(key: &str, value: Option<&str>) {
            match value {
                None => env::remove_var(key),
                Some(value) => env::set_var(key, value),
            }
        }

        setup_env_var("INFURA_API_KEY", infura_api_key);
        setup_env_var("STARTING_BLOCK_NUMBER", starting_block_number);
    }

    fn check_configuration(
        config: Configuration,
        expected_infura_api_key: Option<&str>,
        expected_starting_block_number: Option<u64>,
    ) {
        assert_eq!(config.infura_api_key.as_deref(), expected_infura_api_key);
        assert_eq!(config.starting_block_number, expected_starting_block_number);
    }
}
