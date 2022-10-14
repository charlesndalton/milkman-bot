use anyhow::Result;
use log::{error, info};
use std::{collections::HashMap, time::Duration};
use tokio::time::sleep;

mod configuration;
use crate::configuration::Configuration;

mod ethereum_client;
use crate::ethereum_client::EthereumClient;

mod types;
use crate::types::Swap;

/// Every 15 seconds, do the following:
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
        .expect("Unable to get configuration from the environment variables.");

    let eth_client = EthereumClient::new(&config).expect("Unable to create the Ethereum client.");

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
        sleep(Duration::from_secs(5)).await;

        let range_end = eth_client
            .get_latest_block_number()
            .await
            .expect("Unable to get latest block number.");

        info!("range end: {}", range_end);

        let requested_swaps = eth_client
            .get_requested_swaps(range_start, range_end)
            .await
            .expect("Unable to get latest swaps.");

        info!("Requested swaps: {:?}", requested_swaps);

        for requested_swap in requested_swaps {
            info!("SWAP: {:?}", requested_swap);
            swap_queue.insert(requested_swap.order_contract, requested_swap);
        }

        for requested_swap in swap_queue.clone().values() {
            if is_swap_fulfilled(requested_swap) {
                swap_queue.remove(&requested_swap.order_contract);
            } else {
                // cow_api_client::create_order(requested_swap).await;
            }
        }

        range_start = range_end;
    }
}

fn is_swap_fulfilled(swap: &Swap) -> bool {
    true
}
