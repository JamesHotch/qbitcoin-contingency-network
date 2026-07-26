use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use clap::Parser;
use qbitcoin::{
    block::Block,
    finality::{
        CheckpointProposal, CheckpointSignature, DEVELOPMENT_COMMITTEE_WARNING, DevelopmentSigner,
    },
    params::{ChainParams, Network},
    pow::Sha256dPowV1,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc, time::Duration};

#[derive(Debug, Parser)]
#[command(about = "QBitCoin checkpoint signer")]
struct Arguments {
    #[arg(long, default_value = "regtest")]
    chain: String,
    #[arg(long)]
    signer_id: u16,
    #[arg(long, default_value = "0.0.0.0:7000")]
    bind: SocketAddr,
    #[arg(long, default_value = "http://node1:58332")]
    node_url: String,
    #[arg(long, env = "QBTC_SIGNER_AUTH_TOKEN", hide_env_values = true)]
    auth_token: String,
    #[arg(long, default_value_t = false)]
    local_development: bool,
}

struct SignerState {
    params: ChainParams,
    signer: DevelopmentSigner,
    node_url: String,
    auth_token_hash: qbitcoin::hash::Hash32,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct SignRequest {
    proposal: CheckpointProposal,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = Arguments::parse();
    let params = ChainParams::for_network(parse_network(&arguments.chain)?);
    if !arguments.local_development || params.network == Network::Main {
        return Err("this binary currently permits only non-mainnet local development mode".into());
    }
    if arguments.signer_id >= 11 {
        return Err("development signer-id must be between 0 and 10".into());
    }
    println!("{DEVELOPMENT_COMMITTEE_WARNING}");
    let state = Arc::new(SignerState {
        params,
        signer: DevelopmentSigner::from_id(arguments.signer_id),
        node_url: arguments.node_url,
        auth_token_hash: token_hash(&arguments.auth_token),
        client: reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?,
    });
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/v1/sign", post(sign))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(arguments.bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn sign(
    State(state): State<Arc<SignerState>>,
    headers: HeaderMap,
    Json(request): Json<SignRequest>,
) -> impl IntoResponse {
    if !authenticated(&headers, state.auth_token_hash) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error":"authentication required"})),
        )
            .into_response();
    }
    match validate_and_sign(&state, &request.proposal).await {
        Ok(signature) => (StatusCode::OK, Json(signature)).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":error})),
        )
            .into_response(),
    }
}

async fn validate_and_sign(
    state: &SignerState,
    proposal: &CheckpointProposal,
) -> Result<CheckpointSignature, String> {
    if proposal.chain_id != state.params.chain_id || proposal.network != state.params.network {
        return Err("proposal belongs to another network".to_owned());
    }
    let status: serde_json::Value = state
        .client
        .get(format!("{}/v1/status", state.node_url))
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    if status["chain_id"].as_str() != Some(&proposal.chain_id.to_string())
        || status["consensus_stage"].as_str()
            != serde_json::to_value(proposal.consensus_stage)
                .ok()
                .as_ref()
                .and_then(serde_json::Value::as_str)
    {
        return Err("node status does not match proposal".to_owned());
    }
    let block: Block = state
        .client
        .get(format!(
            "{}/v1/blocks/{}",
            state.node_url, proposal.block_height
        ))
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    if block.hash(&Sha256dPowV1) != proposal.block_hash {
        return Err("node block hash does not match proposal".to_owned());
    }
    state
        .signer
        .sign(proposal)
        .map_err(|error| error.to_string())
}

fn authenticated(headers: &HeaderMap, expected: qbitcoin::hash::Hash32) -> bool {
    let supplied = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(token_hash);
    supplied.is_some_and(|supplied| constant_time_equal(&supplied.0, &expected.0))
}

fn token_hash(token: &str) -> qbitcoin::hash::Hash32 {
    qbitcoin::hash::Hash32::tagged_sha3("QBTC/SIGNER-AUTH/v1", token.as_bytes())
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn parse_network(value: &str) -> Result<Network, &'static str> {
    match value {
        "testnet" => Ok(Network::Testnet),
        "regtest" => Ok(Network::Regtest),
        "main" => Ok(Network::Main),
        _ => Err("chain must be main, testnet, or regtest"),
    }
}
