use axum::{Json, Router, routing::get};
use clap::Parser;
use qbitcoin::{
    params::{ChainParams, Network},
    wallet::Wallet,
};
use std::{net::SocketAddr, path::PathBuf};

#[derive(Debug, Parser)]
#[command(about = "Minimal read-only QBitCoin development wallet service")]
struct Arguments {
    #[arg(long, default_value = "regtest")]
    chain: String,
    #[arg(long, default_value = "/data/dev-wallet.qbtc")]
    wallet: PathBuf,
    #[arg(long, default_value = "0.0.0.0:8080")]
    bind: SocketAddr,
    #[arg(long, env = "QBTC_WALLET_PASSWORD", hide_env_values = true)]
    password: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = Arguments::parse();
    let params = ChainParams::for_network(parse_network(&arguments.chain)?);
    let wallet = if arguments.wallet.exists() {
        Wallet::load_encrypted(&arguments.wallet, arguments.password.as_bytes())?
    } else {
        let wallet = Wallet::create(&params, Some(100));
        wallet.save_encrypted(&arguments.wallet, arguments.password.as_bytes())?;
        wallet
    };
    let summary = wallet.summary(&params)?;
    eprintln!(
        "WARNING: local development wallet service; experimental keys must never hold real value"
    );
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route(
            "/v1/wallet",
            get(move || {
                let summary = summary.clone();
                async move { Json(summary) }
            }),
        );
    let listener = tokio::net::TcpListener::bind(arguments.bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn parse_network(value: &str) -> Result<Network, &'static str> {
    match value {
        "testnet" => Ok(Network::Testnet),
        "regtest" => Ok(Network::Regtest),
        "main" => Ok(Network::Main),
        _ => Err("chain must be main, testnet, or regtest"),
    }
}
