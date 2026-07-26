use clap::Parser;
use qbitcoin::{
    node::{NodeRuntime, serve_http, serve_p2p},
    params::{ChainParams, Network},
};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(about = "QBitCoin experimental full node")]
struct Arguments {
    #[arg(long, default_value = "regtest")]
    chain: String,
    #[arg(long, default_value = "data/node")]
    data_dir: PathBuf,
    #[arg(long, default_value = "127.0.0.1:58332")]
    rpc_bind: SocketAddr,
    #[arg(long, default_value = "0.0.0.0:58333")]
    p2p_bind: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("qbitcoin=info")),
        )
        .init();
    let arguments = Arguments::parse();
    let params = ChainParams::for_network(parse_network(&arguments.chain)?);
    let node = Arc::new(RwLock::new(NodeRuntime::open(params, arguments.data_dir)?));

    tracing::warn!("QBitCoin is unaudited testnet research software; never use it for real value");
    let http = serve_http(Arc::clone(&node), arguments.rpc_bind);
    let p2p = serve_p2p(node, arguments.p2p_bind);
    tokio::select! {
        result = http => result?,
        result = p2p => result?,
        _ = tokio::signal::ctrl_c() => {}
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
