use anyhow::{Context, Result};
use log::debug;
use std::env;

#[derive(Debug, Clone, PartialEq)]
pub struct Configuration {
    pub infura_api_key: String,
    pub starting_block_number: Option<u64>,
}

impl Configuration {
    pub fn get_from_environment() -> Result<Self> {
        let infura_api_key = collect_required_environment_variable("INFURA_API_KEY")?;

        let starting_block_number =
            match collect_optional_environment_variable("STARTING_BLOCK_NUMBER")? {
                Some(block_num) => block_num.parse::<u64>().ok(),
                None => None,
            };

        Ok(Self {
            infura_api_key,
            starting_block_number,
        })
    }
}

fn collect_required_environment_variable(key: &str) -> Result<String> {
    Ok(env::var(key).context(format!("required environment variable {} not set", key))?)
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
        check_configuration(config, "a", None);

        setup_env_vars(Some("a"), Some("123"));
        let config = Configuration::get_from_environment().expect("failed to get");
        check_configuration(config, "a", Some(123));

        setup_env_vars(None, Some("123"));
        assert!(Configuration::get_from_environment().is_err());

        setup_env_vars(Some("a"), None);
        let config = Configuration::get_from_environment().expect("failed to get");
        check_configuration(config, "a", None);
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
        expected_infura_api_key: &str,
        expected_starting_block_number: Option<u64>,
    ) {
        assert_eq!(config.infura_api_key, expected_infura_api_key);
        assert_eq!(config.starting_block_number, expected_starting_block_number);
    }
}
