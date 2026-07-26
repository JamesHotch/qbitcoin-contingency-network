use clap::Parser;
use qbitcoin::{
    block::Block,
    params::{ChainParams, Network},
    pow::Sha256dPowV1,
    wallet::Wallet,
};
use serde::Serialize;
use std::{path::PathBuf, time::Duration};

#[derive(Debug, Parser)]
#[command(about = "QBitCoin CPU development miner")]
struct Arguments {
    #[arg(long, default_value = "regtest")]
    chain: String,
    #[arg(long)]
    wallet: PathBuf,
    #[arg(long, env = "QBTC_WALLET_PASSWORD", hide_env_values = true)]
    password: String,
    #[arg(long, default_value = "http://127.0.0.1:58332")]
    node_url: String,
    #[arg(long, default_value_t = 1)]
    blocks: u64,
}

#[derive(Serialize)]
struct TemplateRequest {
    miner_public_keys: qbitcoin::crypto::HybridPublicKeys,
    timestamp: Option<u64>,
    extra_nonce: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = Arguments::parse();
    let params = ChainParams::for_network(parse_network(&arguments.chain)?);
    let wallet = Wallet::load_encrypted(&arguments.wallet, arguments.password.as_bytes())?;
    let public_keys = wallet.primary_public_keys(&params)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    for index in 0..arguments.blocks {
        let response = client
            .post(format!("{}/v1/mining/template", arguments.node_url))
            .json(&TemplateRequest {
                miner_public_keys: public_keys.clone(),
                timestamp: None,
                extra_nonce: Some(format!("cpu-{index}")),
            })
            .send()
            .await?
            .error_for_status()?;
        let mut block: Block = response.json().await?;
        let hash = block.mine(&Sha256dPowV1, u32::MAX)?;
        let result = client
            .post(format!("{}/v1/blocks", arguments.node_url))
            .json(&block)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        println!("mined {hash}: {result}");
    }
    Ok(())
}

fn parse_network(value: &str) -> Result<Network, &'static str> {
    match value {
        "main" => Ok(Network::Main),
        "testnet" => Ok(Network::Testnet),
        "regtest" => Ok(Network::Regtest),
        _ => Err("chain must be main, testnet, or regtest"),
    }
}
