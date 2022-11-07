use anyhow::Result;
use log::{error, info};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::sleep;

mod environment;
use crate::environment::Environment;

mod swap;
use crate::swap::Swap;

mod cow_api_client;
mod ethereum_client;
use crate::ethereum_client::SwapState;

pub type SwapQueue = Arc<Mutex<VecDeque<Swap>>>;

pub const MILKMAN_ADDRESS: &str = "0x9d763Cca6A8551283478CeC44071d72Ec3FD58Cb";
pub const MILKMAN_STATE_HELPER_ADDRESS: &str = "0xE549Bc5c6023E68d4e5af6AfacC45a0dB67bF01C";

/// Wait for requested swaps, pair those swaps, and unpair and re-pair swaps that
/// don't get executed.
///
/// The bot is split into three threads of execution:
/// - wait for requested swaps and enqueue them on the `requested_swap_queue`
/// - dequeue requested swaps from the `requested_swap_queue,` execute them,
///   and push them to the `awaiting_finalization_swap_queue`
/// - dequeue swaps from the `awaiting_finalization_swap_queue`, check if they
///   have been executed, and either:
///     - if executed, delete them from the `awaitingFinalizationSwapQueue`
///     - if not executed, unpair them and enqueue them to the `requestedSwapQueue`
#[tokio::main]
async fn main() {
    env_logger::init();

    info!("=== MILKMAN BOT STARTING ===");

    let env = Arc::new(Environment::collect().expect("failed to collect environment"));

    let requested_swap_queue = Arc::new(Mutex::new(VecDeque::<Swap>::new()));
    let awaiting_finalization_swap_queue = Arc::new(Mutex::new(VecDeque::<Swap>::new()));

    let mut handles = Vec::new();

    let thread_env = Arc::clone(&env);
    let thread_requested_swap_queue = Arc::clone(&requested_swap_queue);
    let thread_awaiting_finalization_swap_queue = Arc::clone(&awaiting_finalization_swap_queue);
    handles.push(tokio::task::spawn(async move {
        enqueue_requested_swaps(
            thread_env,
            thread_requested_swap_queue,
            thread_awaiting_finalization_swap_queue,
        )
        .await
        .expect("failed to enqueue requested swaps");
    }));

    let thread_env = Arc::clone(&env);
    let thread_requested_swap_queue = Arc::clone(&requested_swap_queue);
    let thread_awaiting_finalization_swap_queue = Arc::clone(&awaiting_finalization_swap_queue);
    handles.push(tokio::task::spawn(async move {
        execute_requested_swaps(
            thread_env,
            thread_requested_swap_queue,
            thread_awaiting_finalization_swap_queue,
        )
        .await
        .expect("failed to execute requested swaps");
    }));

    let thread_env = Arc::clone(&env);
    let thread_requested_swap_queue = Arc::clone(&requested_swap_queue);
    let thread_awaiting_finalization_swap_queue = Arc::clone(&awaiting_finalization_swap_queue);
    handles.push(tokio::task::spawn(async move {
        finalize_swaps(
            thread_env,
            thread_requested_swap_queue,
            thread_awaiting_finalization_swap_queue,
        )
        .await
        .expect("failed to finalize swaps");
    }));

    for handle in handles {
        handle.await.expect("thread threw error");
    }
}

async fn enqueue_requested_swaps(
    env: Arc<Environment>,
    requested_swap_queue: SwapQueue,
    awaiting_finalization_swap_queue: SwapQueue,
) -> Result<()> {
    info!("starting to enqueue requested swaps");

    let mut starting_block_number = env
        .starting_block_number
        .unwrap_or(ethereum_client::get_latest_block_number(Arc::clone(&env)).await?);

    info!("starting block number : {:?}", starting_block_number);

    let milkman = ethereum_client::get_milkman(Arc::clone(&env)).await?;

    info!("got milkman");

    let swap_requested_filter = milkman
        .swap_requested_filter()
        .filter
        .from_block(starting_block_number);

    info!("filter : {:?}", swap_requested_filter);

    // let ethers_client =
    //     ethereum_client::get_ethers_client(&env.infura_api_key, &env.keeper_private_key).await?;

    // let mut swap_request_stream = ethers_client.watch(&swap_requested_filter).await?;

    loop {
        let current_block_number =
            ethereum_client::get_latest_block_number(Arc::clone(&env)).await?;

        let swap_requests = milkman
            .swap_requested_filter()
            .from_block(starting_block_number)
            .to_block(current_block_number)
            .query()
            .await?;

        for swap_request in swap_requests {
            info!("swap request - {:?}", swap_request);

            // disregard dust
            if swap_request.amount_in.as_u128() < 100 {
                info!("swap doesn't have enough amount_in");
                continue;
            }

            let swap_state =
                ethereum_client::get_swap_state(&swap_request.swap_id, Arc::clone(&env)).await?;

            info!("swap state: {:?}", swap_state);

            let queue_to_push_to = if swap_state == SwapState::REQUESTED {
                &requested_swap_queue
            } else {
                &awaiting_finalization_swap_queue
            };

            push_swap_to_queue(
                Swap {
                    swap_id: swap_request.swap_id,
                    user: swap_request.user,
                    receiver: swap_request.receiver,
                    from_token: swap_request.from_token,
                    to_token: swap_request.to_token,
                    amount_in: swap_request.amount_in,
                    price_checker: swap_request.price_checker,
                    nonce: swap_request.nonce,
                },
                queue_to_push_to,
            );
        }

        starting_block_number = current_block_number;

        sleep(Duration::from_secs(60)).await;
    }
}

async fn execute_requested_swaps(
    env: Arc<Environment>,
    requested_swap_queue: SwapQueue,
    awaiting_finalization_swap_queue: SwapQueue,
) -> Result<()> {
    loop {
        sleep(Duration::from_secs(10)).await;

        while let Some(swap_request) = pop_front_from_queue(&requested_swap_queue) {
            info!("dequeued swap request with ID – {:?}", swap_request.swap_id);

            match pair_swap(&swap_request, Arc::clone(&env)).await {
                Ok(_) => {
                    info!(
                        "successfully paired swap request with ID - {:?}",
                        swap_request.swap_id
                    );
                    push_swap_to_queue(swap_request, &awaiting_finalization_swap_queue);
                }
                Err(err) => {
                    error!(
                        "could not successfully pair swap with ID - {:?}",
                        swap_request.swap_id
                    );
                    error!("ERROR: {:?}", err);
                    push_swap_to_queue(swap_request, &requested_swap_queue);
                }
            }
        }
    }

    async fn pair_swap(swap_request: &Swap, env: Arc<Environment>) -> Result<()> {
        let fee_and_quote = cow_api_client::get_fee_and_quote(
            swap_request.from_token,
            swap_request.to_token,
            swap_request.amount_in,
        )
        .await?;

        info!("retrieved quote – {:?}", fee_and_quote);

        // TODO: make slippage configurable
        let buy_amount_with_fee_after_slippage = fee_and_quote.buy_amount_after_fee * 99 / 100;

        let valid_to =
            ethereum_client::get_current_timestamp(Arc::clone(&env)).await? + 60 * 60 * 24; // 1 day expiry

        let sell_amount = swap_request.amount_in - fee_and_quote.fee_amount;

        let order_uid = cow_api_client::create_order(
            swap_request.from_token,
            swap_request.to_token,
            sell_amount,
            buy_amount_with_fee_after_slippage,
            valid_to,
            fee_and_quote.fee_amount,
            swap_request.receiver,
        )
        .await?;

        info!("created order, UID = {:?}", order_uid);

        ethereum_client::pair_swap(
            swap_request,
            &fee_and_quote,
            valid_to,
            buy_amount_with_fee_after_slippage,
            Arc::clone(&env),
        )
        .await?;

        Ok(())
    }
}

async fn finalize_swaps(
    env: Arc<Environment>,
    requested_swap_queue: SwapQueue,
    awaiting_finalization_swap_queue: SwapQueue,
) -> Result<()> {
    loop {
        sleep(Duration::from_secs(120)).await;

        while let Some(swap_request) = pop_front_from_queue(&awaiting_finalization_swap_queue) {
            info!(
                "dequeued swap to finalize with ID – {:?}",
                swap_request.swap_id
            );

            let swap_state =
                ethereum_client::get_swap_state(&swap_request.swap_id, Arc::clone(&env)).await?;

            match swap_state {
                SwapState::PAIRED_AND_EXECUTED => {
                    info!("swap with ID {:?} has been executed – removing from queue", swap_request.swap_id);
                }
                SwapState::PAIRED_AND_UNPAIRABLE => {
                    info!("swap with ID {:?} is unpairable – unpairing and pushing to requested swap queue", swap_request.swap_id);
                    ethereum_client::unpair_swap(&swap_request.swap_id, Arc::clone(&env)).await?;
                    push_swap_to_queue(swap_request, &requested_swap_queue);
                }
                SwapState::PAIRED => {
                    info!(
                        "swap with ID {:?} is paired but not yet executed – requeing",
                        swap_request.swap_id
                    );
                    push_swap_to_queue(swap_request, &awaiting_finalization_swap_queue);
                }
                _ => {
                    panic!("swap is in a wrong state: {:?}", swap_state);
                }
            }
        }
    }
}

fn push_swap_to_queue(swap: Swap, queue: &SwapQueue) {
    let mut mutable_queue = queue.lock().unwrap();
    mutable_queue.push_back(swap);
}

fn pop_front_from_queue(queue: &SwapQueue) -> Option<Swap> {
    let mut mutable_queue = queue.lock().unwrap();
    mutable_queue.pop_front()
}