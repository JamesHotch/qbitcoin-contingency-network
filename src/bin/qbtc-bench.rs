use clap::Parser;
use qbitcoin::{
    crypto::{DomainTag, HybridKeyPair, RustCryptoProvider, verify_hybrid},
    params::{ChainParams, Network},
};
use serde::Serialize;
use std::time::Instant;

#[derive(Debug, Parser)]
#[command(about = "QBitCoin cryptography and weight benchmark")]
struct Arguments {
    #[arg(long, default_value_t = 25)]
    iterations: u64,
}

#[derive(Serialize)]
struct Report {
    iterations: u64,
    schnorr_public_key_bytes: usize,
    ml_dsa_public_key_bytes: usize,
    schnorr_signature_bytes: usize,
    ml_dsa_signature_bytes: usize,
    average_hybrid_sign_micros: u128,
    average_hybrid_verify_micros: u128,
    configured_max_transaction_weight: u64,
    configured_max_block_weight: u64,
    configured_max_signature_cost: u64,
    warning: &'static str,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = Arguments::parse();
    if arguments.iterations == 0 {
        return Err("iterations must be positive".into());
    }
    let params = ChainParams::for_network(Network::Regtest);
    let keys = HybridKeyPair::generate(None);
    let public = keys.public_keys();
    let message = b"QBitCoin benchmark message";
    let mut signatures = Vec::with_capacity(arguments.iterations as usize);

    let signing_started = Instant::now();
    for _ in 0..arguments.iterations {
        signatures.push(keys.sign_hybrid(
            &params.chain_id,
            DomainTag::TransactionSignature,
            message,
        )?);
    }
    let sign_micros = signing_started.elapsed().as_micros();

    let verification_started = Instant::now();
    for signature in &signatures {
        verify_hybrid(
            &RustCryptoProvider,
            &params.chain_id,
            DomainTag::TransactionSignature,
            message,
            &public,
            signature,
        )?;
    }
    let verify_micros = verification_started.elapsed().as_micros();
    let first = &signatures[0];
    let report = Report {
        iterations: arguments.iterations,
        schnorr_public_key_bytes: public.schnorr.len(),
        ml_dsa_public_key_bytes: public.ml_dsa.len(),
        schnorr_signature_bytes: first.schnorr.as_ref().map_or(0, Vec::len),
        ml_dsa_signature_bytes: first.ml_dsa.as_ref().map_or(0, Vec::len),
        average_hybrid_sign_micros: sign_micros / u128::from(arguments.iterations),
        average_hybrid_verify_micros: verify_micros / u128::from(arguments.iterations),
        configured_max_transaction_weight: params.max_transaction_weight,
        configured_max_block_weight: params.max_block_weight,
        configured_max_signature_cost: params.max_signature_verification_cost,
        warning: "Single-host research benchmark; not a production parameter study.",
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
