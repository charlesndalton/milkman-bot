use crate::cow_api_client::FeeAndQuote;
use crate::environment::Environment;
use crate::swap::Swap;
use crate::{MILKMAN_ADDRESS, MILKMAN_STATE_HELPER_ADDRESS};
use anyhow::{anyhow, Result};
use ethers::prelude::*;
use url::Url;

use ethers_flashbots::*;
use std::sync::Arc;

abigen!(
    RawMilkman,
    "./abis/Milkman.json", // ethers doesn't allow you to input the address dynamically
    event_derives(serde::Deserialize, serde::Serialize),
);

abigen!(RawMilkmanStateHelper, "./abis/MilkmanStateHelper.json");

type EthersMiddleware = SignerMiddleware<Provider<Http>, LocalWallet>;
pub type EthersClient = Arc<EthersMiddleware>;

pub type Milkman = RawMilkman<EthersMiddleware>;
pub type MilkmanStateHelper = RawMilkmanStateHelper<EthersMiddleware>;

#[derive(Debug, PartialEq)]
pub enum SwapState {
    NULL,
    REQUESTED,
    PAIRED,
    PAIRED_AND_UNPAIRABLE,
    PAIRED_AND_EXECUTED,
}

pub async fn get_milkman(env: Arc<Environment>) -> Result<Milkman> {
    let client = get_ethers_client(&env.infura_api_key, &env.keeper_private_key).await?;

    Ok(RawMilkman::new(MILKMAN_ADDRESS.parse::<Address>()?, client))
}

pub async fn get_milkman_state_helper(env: Arc<Environment>) -> Result<MilkmanStateHelper> {
    let client = get_ethers_client(&env.infura_api_key, &env.keeper_private_key).await?;

    Ok(RawMilkmanStateHelper::new(
        MILKMAN_STATE_HELPER_ADDRESS.parse::<Address>()?,
        client,
    ))
}

pub async fn get_swap_state(swap_id: &[u8; 32], env: Arc<Environment>) -> Result<SwapState> {
    let state_helper = get_milkman_state_helper(env).await?;

    let raw_swap_state = state_helper.get_state(*swap_id).call().await?;

    Ok(match raw_swap_state {
        0 => SwapState::NULL,
        1 => SwapState::REQUESTED,
        2 => SwapState::PAIRED,
        3 => SwapState::PAIRED_AND_UNPAIRABLE,
        4 => SwapState::PAIRED_AND_EXECUTED,
        _ => panic!("Something is seriously wrong here – swap state should be between 0-4 but contract returned {:?}", raw_swap_state)
    })
}

pub async fn pair_swap(
    swap_request: &Swap,
    fee_and_quote: &FeeAndQuote,
    valid_to: u64,
    buy_amount_with_fee_after_slippage: U256,
    env: Arc<Environment>,
) -> Result<()> {
    let milkman = get_milkman(Arc::clone(&env)).await?;

    let sell_amount = swap_request.amount_in - fee_and_quote.fee_amount;

    milkman
        .pair_swap(
            raw_milkman::Data {
                sell_token: swap_request.from_token,
                buy_token: swap_request.to_token,
                receiver: swap_request.receiver,
                sell_amount,
                buy_amount: buy_amount_with_fee_after_slippage,
                valid_to: valid_to.try_into()?,
                app_data: str_to_bytes32(
                    "2B8694ED30082129598720860E8E972F07AA10D9B81CAE16CA0E2CFB24743E24",
                ),
                fee_amount: fee_and_quote.fee_amount,
                kind: str_to_bytes32(
                    "f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee346775",
                ),
                partially_fillable: false,
                sell_token_balance: str_to_bytes32(
                    "5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9",
                ),
                buy_token_balance: str_to_bytes32(
                    "5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9",
                ),
            },
            swap_request.user,
            "0x0000000000000000000000000000000000000000".parse::<Address>()?,
            swap_request.nonce,
        )
        .send()
        .await?
        .await?;

    Ok(())
}

pub async fn unpair_swap(
    swap_id: &[u8; 32],
    env: Arc<Environment>,
) -> Result<()> {
    let milkman = get_milkman(Arc::clone(&env)).await?;

    milkman
        .unpair_swap(*swap_id)
        .send()
        .await?
        .await?;

    Ok(())

}

pub async fn get_latest_block_number(env: Arc<Environment>) -> Result<u64> {
    let latest_block = get_latest_block(env).await?;

    latest_block
        .number
        .ok_or(anyhow!("unable to fetch latest block"))
        .map(|block_num: U64| block_num.try_into().unwrap()) // U64 -> u64 should never fail
}

pub async fn get_current_timestamp(env: Arc<Environment>) -> Result<u64> {
    let latest_block = get_latest_block(env).await?;

    Ok(latest_block.timestamp.as_u64())
}

async fn get_latest_block(env: Arc<Environment>) -> Result<Block<H256>> {
    let client = get_ethers_client(&env.infura_api_key, &env.keeper_private_key).await?;

    client
        .get_block(BlockNumber::Latest)
        .await?
        .ok_or(anyhow!("error fetching latest block"))
}

/// Create an ethers client object that can be used to read data from and send
/// transactions to Ethereum.
///
/// Because these objects are cheap to create, we opt to create them wherever we
/// need one instead of creating one at the start and trying to pass it around.
pub async fn get_ethers_client(
    infura_api_key: &str,
    keeper_private_key: &str,
) -> Result<EthersClient> {
    // let provider =
    //     Provider::<Ws>::connect(format!("wss://mainnet.infura.io/ws/v3/{}", infura_api_key))
    //         .await?;
    let infura_url = format!("https://mainnet.infura.io/v3/{}", infura_api_key);
    let provider = Provider::<Http>::try_from(infura_url)?;
    let wallet: LocalWallet = keeper_private_key.parse()?;
    let client = SignerMiddleware::new(provider, wallet);
    // let client = SignerMiddleware::new(
    //     FlashbotsMiddleware::new(
    //         provider,
    //         Url::parse("https://relay.flashbots.net")?,
    //         wallet.clone(),
    //     ),
    //     wallet,
    // );
    Ok(Arc::new(client))
}

fn str_to_bytes32(_str: &str) -> [u8; 32] {
    hex::decode(_str).unwrap()[0..32].try_into().unwrap()
}
