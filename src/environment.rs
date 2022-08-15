use anyhow::{Context, Result};
use log::debug;
use std::env;

#[derive(Debug)]
pub struct Environment {
    pub infura_api_key: String,
    pub keeper_private_key: String,
    pub starting_block_number: Option<u64>,
}

impl Environment {
    pub fn collect() -> Result<Self> {
        let infura_api_key = collect_required_environment_variable("INFURA_API_KEY")?;
        let keeper_private_key = collect_required_environment_variable("KEEPER_PRIVATE_KEY")?;
        let starting_block_number =
            match collect_optional_environment_variable("STARTING_BLOCK_NUMBER")? {
                None => None,
                Some(value) => value.parse::<u64>().ok(), // Return `None` if value can't be parsed into a u64
            };

        Ok(Self {
            infura_api_key,
            keeper_private_key,
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
        Err(err) => {
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
    fn test_collect() {
        setup_env_vars(Some("a"), Some("b"), Some("c"));
        let env = Environment::collect().expect("failed to collect");
        check_environment(env, "a", "b", None);

        setup_env_vars(Some("a"), Some("b"), Some("123"));
        let env = Environment::collect().expect("failed to collect");
        check_environment(env, "a", "b", Some(123));

        setup_env_vars(None, Some("b"), Some("123"));
        assert!(Environment::collect().is_err());

        setup_env_vars(Some("a"), None, Some("123"));
        assert!(Environment::collect().is_err());

        setup_env_vars(Some("a"), Some("b"), None);
        let env = Environment::collect().expect("failed to collect");
        check_environment(env, "a", "b", None);
    }

    fn setup_env_vars(
        infura_api_key: Option<&str>,
        keeper_private_key: Option<&str>,
        starting_block_number: Option<&str>,
    ) {
        fn setup_env_var(key: &str, value: Option<&str>) {
            match value {
                None => env::remove_var(key),
                Some(value) => env::set_var(key, value),
            }
        }

        setup_env_var("INFURA_API_KEY", infura_api_key);
        setup_env_var("KEEPER_PRIVATE_KEY", keeper_private_key);
        setup_env_var("STARTING_BLOCK_NUMBER", starting_block_number);
    }

    fn check_environment(
        environment: Environment,
        expected_infura_api_key: &str,
        expected_keeper_private_key: &str,
        expected_starting_block_number: Option<u64>,
    ) {
        assert_eq!(environment.infura_api_key, expected_infura_api_key);
        assert_eq!(environment.keeper_private_key, expected_keeper_private_key);
        assert_eq!(
            environment.starting_block_number,
            expected_starting_block_number
        );
    }
}
