use ethers::{abi::{AbiEncode, AbiDecode}, prelude::*, utils::keccak256};
use std::env;
use eyre::Result;
use std::sync::Arc;
use serde_json::{Value};

abigen!(
    CowAnywhere,
    "./src/abis/CowAnywhere.json",
    event_derives(serde::Deserialize, serde::Serialize)
);

pub type BlockchainClient = Arc<SignerMiddleware<Provider::<Ws>, LocalWallet>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let infura_api_key = env::var("INFURA_API_KEY").expect("INFURA_API_KEY not set");
    let keeper_private_key = env::var("KEEPER_PRIVATE_KEY").expect("KEEPER_PRIVATE_KEY not set");

    let client = get_blockchain_client(&infura_api_key, &keeper_private_key).await?;

    let last_block = client.get_block(BlockNumber::Latest).await?.unwrap();

    //println!("last_block: {:?}", last_block);

    let swap_requested_filter = Filter::new()
        .from_block(15136972)
        .address("0x5F4bd1b3667127Bf44beBBa9e5d736B65A1677E5".parse::<Address>()?)
        .topic0(ValueOrArray::Value(H256::from(keccak256("SwapRequested(address,address,address,address,uint256,address,uint256)"))));

    let contract = CowAnywhere::new("0x5F4bd1b3667127Bf44beBBa9e5d736B65A1677E5".parse::<Address>()?, Arc::clone(&client));
    let logs = contract.swap_requested_filter().from_block(0u64).query().await?;
    println!("{:?}", logs);

    let event_filter = contract.swap_requested_filter().from_block(0u64);

    let mut stream = event_filter.subscribe().await?;
    //let mut stream = client.subscribe_logs(&swap_requested_filter).await?;

    while let Some(log) = stream.next().await {
        println!("{:?}", log);
        let log = log?;
        let quote = get_fee_and_quote(log.from_token, log.to_token, log.amount_in).await?;
        println!("QUOTE: {:?}", quote);
        let sell_amount = log.amount_in - quote.fee_amount;
        let buy_amount_with_fee_after_slippage = quote.buy_amount_after_fee * 995 / 1000;
        let valid_to = last_block.timestamp.as_u64() + 60*60*24;
        let mut order_uid = create_order(log.from_token, log.to_token, sell_amount, buy_amount_with_fee_after_slippage, valid_to, quote.fee_amount, log.receiver).await?; 
        order_uid.remove(0); // 0x
        order_uid.remove(0);

        let call = contract.sign_order_uid(hex::decode(order_uid)?.into(), cowanywhere_mod::Data { sell_token: log.from_token, buy_token: log.to_token, receiver: log.receiver, sell_amount: sell_amount, buy_amount: buy_amount_with_fee_after_slippage, valid_to: valid_to.try_into()?, app_data: hex::decode("2B8694ED30082129598720860E8E972F07AA10D9B81CAE16CA0E2CFB24743E24")?[0..32].try_into().unwrap(), fee_amount: quote.fee_amount, kind: str_to_bytes32("f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee346775"), partially_fillable: false, sell_token_balance: str_to_bytes32("5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9"), buy_token_balance: str_to_bytes32("5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9") }, "0x711d1D8E8B2b468c92c202127A2BBFEFC14bf105".parse::<Address>()?, "0x0000000000000000000000000000000000000000".parse::<Address>()?);

        println!("{:?}", call.calldata().unwrap());

        let _receipt = call.send().await?.await?;
    }

    Ok(())
}

async fn get_blockchain_client(infura_api_key: &str, keeper_private_key: &str) -> Result<BlockchainClient> {
    let provider =
        Provider::<Ws>::connect(format!("wss://mainnet.infura.io/ws/v3/{}", infura_api_key))
            .await?;
    let wallet: LocalWallet = keeper_private_key.parse()?;
    let client = SignerMiddleware::new(provider, wallet);
    Ok(Arc::new(client))
} 

#[derive(Debug)]
struct FeeAndQuote {
    fee_amount: U256,
    buy_amount_after_fee: U256,
}

pub async fn get_fee_and_quote(sell_token: Address, buy_token: Address, sell_amount_before_fee: U256) -> Result<FeeAndQuote> {
    let client = reqwest::Client::new();
    println!("{:?}", sell_amount_before_fee.to_string());
    let res = client.get("https://api.cow.fi/mainnet/api/v1/feeAndQuote/sell")
        .query(&[("sellToken", address_to_string(sell_token))])
        .query(&[("buyToken", address_to_string(buy_token))])
        .query(&[("sellAmountBeforeFee", sell_amount_before_fee.as_u128())])
        .send()
        .await?
        .json::<Value>()
        .await?;
    
    println!("{:?}", res);

    let fee_amount = res["fee"]["amount"].as_str().unwrap().to_owned();
    let buy_amount_after_fee = res["buyAmountAfterFee"].as_str().unwrap().to_owned();

    Ok(FeeAndQuote { fee_amount: fee_amount.parse::<u128>()?.into(), buy_amount_after_fee: buy_amount_after_fee.parse::<u128>()?.into() } )
}

async fn create_order(sell_token: Address, buy_token: Address, sell_amount: U256, buy_amount: U256, valid_to: u64, fee_amount: U256, receiver: Address) -> Result<String> {
    let client = reqwest::Client::new();
    let order_uid = client
        .post("https://api.cow.fi/mainnet/api/v1/orders")
        .json(&serde_json::json!({
            "sellToken": address_to_string(sell_token),
            "buyToken": address_to_string(buy_token),
            "sellAmount": sell_amount.to_string(),
            "buyAmount": buy_amount.to_string(),
            "validTo": valid_to,
            "appData": "0x2B8694ED30082129598720860E8E972F07AA10D9B81CAE16CA0E2CFB24743E24",
            "feeAmount": fee_amount.to_string(),
            "kind": "sell",
            "partiallyFillable": false,
            "receiver": address_to_string(receiver),
            "signature": "0x5F4bd1b3667127Bf44beBBa9e5d736B65A1677E5",
            "from": "0x5F4bd1b3667127Bf44beBBa9e5d736B65A1677E5",
            "sellTokenBalance": "erc20",
            "buyTokenBalance": "erc20",
            "signingScheme": "presign"
        }))
        .send()
        .await?
        .json::<Value>()
        .await?;
    println!("{:?}", order_uid);

    Ok(order_uid.as_str().unwrap().to_owned())
}

fn address_to_string(address: Address) -> String {
    "0x".to_owned() + &hex::encode(address.to_fixed_bytes())
}

fn str_to_bytes32(_str: &str) -> [u8; 32] {
    hex::decode(_str).unwrap()[0..32].try_into().unwrap()
}
