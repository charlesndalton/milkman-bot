use anyhow::Result;
use log::{error, info};
use std::{collections::HashMap, time::Duration};
use tokio::time::sleep;

mod configuration;
use crate::configuration::Configuration;

mod ethereum_client;
use crate::ethereum_client::EthereumClient;

mod cow_api_client;
use crate::cow_api_client::CowAPIClient;

mod encoder;

mod types;
use crate::types::Swap;

/// Every x seconds, do the following:
/// - check for new Milkman swap requests, and enqueue them into a swap queue
/// - check if items in the swap queue have already been fulfilled
///     - if the swap has already been fulfilled, dequeue it
///     - if the swap hasn't been fulfilled, create an order via the CoW API
///
/// This implies that multiple API orders can be created for a single swap request.
/// We accept this trade-off because it gives us additional simplicity, and the
/// marginal cloud compute cost to CoW is likely to be very small.
#[tokio::main]
async fn main() {
    env_logger::init();

    info!("=== MILKMAN BOT STARTING ===");

    let config = Configuration::get_from_environment()
        .expect("Unable to get configuration from the environment variables."); // .expect() because every decision to panic should be conscious, not just triggered by a `?` that we didn't think about

    let eth_client = EthereumClient::new(&config).expect("Unable to create the Ethereum client.");
    let cow_api_client = CowAPIClient::new(&config);

    // During development, I found Infura's WebSockets endpoint to sometimes miss
    // swaps, so we pull in requested swaps by quering through a series of ranges.
    // For example, if the user passes in a starting block number of 10 and the
    // current block number is 20, the initial request would pull from 10 to 20.
    // After that first request, `range_start` would be set to 20. By the time
    // of the second request, we will query the current block number, let's say
    // 22, and so query from 20 to 22. This is repeated in an infinite loop.
    let mut range_start = config.starting_block_number.unwrap_or(
        eth_client
            .get_latest_block_number()
            .await
            .expect("Unable to get latest block number before starting."),
    );

    info!("range start: {}", range_start);

    let mut swap_queue = HashMap::new();

    loop {
        sleep(Duration::from_secs(config.polling_frequency_secs)).await;

        let range_end = eth_client
            .get_latest_block_number()
            .await
            .expect("Unable to get latest block number."); // should we panic here if we can't get it? another option would be continuing in the loop, but then we might not observe that the bot is really `down`

        info!("range end: {}", range_end);

        let requested_swaps = match eth_client.get_requested_swaps(range_start, range_end).await {
            Ok(swaps) => swaps,
            Err(err) => {
                error!("unable to get requested swaps – {:?}", err);
                continue;
            }
        };

        info!("Requested swaps: {:?}", requested_swaps);

        for requested_swap in requested_swaps {
            info!("SWAP: {:?}", requested_swap);
            swap_queue.insert(requested_swap.order_contract, requested_swap);
        }

        for requested_swap in swap_queue.clone().values() {
            let is_swap_fulfilled = match is_swap_fulfilled(requested_swap, &eth_client).await {
                Ok(res) => res,
                Err(err) => {
                    error!("unable to determine if swap was fulfilled – {:?}", err);
                    continue;
                }
            };

            if is_swap_fulfilled {
                swap_queue.remove(&requested_swap.order_contract);
            } else {
                let quote = match cow_api_client
                    .get_fee_and_quote(
                        requested_swap.from_token,
                        requested_swap.to_token,
                        requested_swap.amount_in,
                    )
                    .await
                {
                    Ok(res) => res,
                    Err(err) => {
                        error!("unable to fetch quote - {:?}", err);
                        continue;
                    }
                };

                let sell_amount_after_fees = requested_swap.amount_in - quote.fee_amount;
                let buy_amount_after_fees_and_slippage = quote.buy_amount_after_fee * 995 / 1000;

                let time = match eth_client.get_chain_timestamp().await {
                    Ok(res) => res,
                    Err(err) => {
                        error!("unable to get chain timestamp – {:?}", err);
                        continue;
                    }
                };
                let valid_to = time + (60 * 60 * 24);

                let eip_1271_signature = encoder::get_eip_1271_signature(
                    requested_swap.from_token,
                    requested_swap.to_token,
                    requested_swap.receiver,
                    sell_amount_after_fees,
                    buy_amount_after_fees_and_slippage,
                    valid_to,
                    quote.fee_amount,
                    requested_swap.order_creator,
                    requested_swap.price_checker,
                    &requested_swap.price_checker_data,
                );

                info!("SIGNATURE: {:?}", eip_1271_signature.to_string());

                match cow_api_client
                    .create_order(
                        requested_swap.order_contract,
                        requested_swap.from_token,
                        requested_swap.to_token,
                        sell_amount_after_fees,
                        buy_amount_after_fees_and_slippage,
                        valid_to,
                        quote.fee_amount,
                        requested_swap.receiver,
                        &eip_1271_signature,
                    )
                    .await
                {
                    Ok(_) => (),
                    Err(err) => error!("unable to create order via CoW API – {:?}", err),
                };
            }
        }

        range_start = range_end;
    }
}

async fn is_swap_fulfilled(swap: &Swap, eth_client: &EthereumClient) -> Result<bool> {
    // if all `from` tokens are gone, the swap must have completed or cancelled
    Ok(eth_client
        .get_balance_of(swap.from_token, swap.order_contract)
        .await?
        .is_zero())
}
