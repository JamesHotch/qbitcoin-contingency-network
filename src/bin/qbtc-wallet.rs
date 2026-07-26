use clap::{Parser, Subcommand};
use qbitcoin::{
    amount::Amount,
    params::{ChainParams, Network},
    transaction::Transaction,
    wallet::{Wallet, WalletUtxo},
};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Parser)]
#[command(about = "Encrypted QBitCoin testnet CLI wallet")]
struct Arguments {
    #[arg(long, default_value = "regtest", global = true)]
    chain: String,
    #[arg(
        long,
        env = "QBTC_WALLET_PASSWORD",
        global = true,
        hide_env_values = true
    )]
    password: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Create {
        #[arg(long)]
        wallet: PathBuf,
        #[arg(long)]
        recovery_delay: Option<u32>,
    },
    Address {
        #[arg(long)]
        wallet: PathBuf,
    },
    Summary {
        #[arg(long)]
        wallet: PathBuf,
    },
    Backup {
        #[arg(long)]
        wallet: PathBuf,
        #[arg(long)]
        destination: PathBuf,
    },
    Restore {
        #[arg(long)]
        backup: PathBuf,
        #[arg(long)]
        wallet: PathBuf,
    },
    CreateTransaction {
        #[arg(long)]
        wallet: PathBuf,
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Broadcast {
        #[arg(long)]
        transaction: PathBuf,
        #[arg(long, default_value = "http://127.0.0.1:58332")]
        node_url: String,
    },
    FeeEstimate {
        #[arg(long)]
        inputs: u64,
        #[arg(long)]
        outputs: u64,
        #[arg(long, default_value_t = 1)]
        atoms_per_weight: u64,
    },
}

#[derive(Debug, Deserialize)]
struct PaymentRequest {
    available: Vec<WalletUtxo>,
    recipient_address: String,
    amount_atoms: u64,
    fee_atoms: u64,
    coin_control: Option<BTreeSet<qbitcoin::transaction::OutPoint>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = Arguments::parse();
    let params = ChainParams::for_network(parse_network(&arguments.chain)?);
    match arguments.command {
        Command::Create {
            wallet,
            recovery_delay,
        } => {
            let password = required_password(arguments.password.as_deref())?;
            let created = Wallet::create(&params, recovery_delay);
            created.save_encrypted(&wallet, password)?;
            println!("{}", created.primary_address(&params)?);
            eprintln!(
                "WARNING: experimental cryptography; protect the encrypted backup and do not store real value"
            );
        }
        Command::Address { wallet } => {
            let loaded = load(&wallet, arguments.password.as_deref())?;
            println!("{}", loaded.primary_address(&params)?);
        }
        Command::Summary { wallet } => {
            let loaded = load(&wallet, arguments.password.as_deref())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&loaded.summary(&params)?)?
            );
        }
        Command::Backup {
            wallet,
            destination,
        } => {
            Wallet::backup(wallet, destination)?;
        }
        Command::Restore { backup, wallet } => {
            let password = required_password(arguments.password.as_deref())?;
            let restored = Wallet::load_encrypted(&backup, password)?;
            restored.summary(&params)?;
            restored.save_encrypted(wallet, password)?;
        }
        Command::CreateTransaction {
            wallet,
            request,
            output,
        } => {
            let loaded = load(&wallet, arguments.password.as_deref())?;
            let payment: PaymentRequest = serde_json::from_slice(&fs::read(request)?)?;
            let transaction = loaded.create_payment(
                &params,
                &payment.available,
                &payment.recipient_address,
                Amount::from_atoms(payment.amount_atoms)?,
                Amount::from_atoms(payment.fee_atoms)?,
                payment.coin_control.as_ref(),
            )?;
            fs::write(output, serde_json::to_vec_pretty(&transaction)?)?;
        }
        Command::Broadcast {
            transaction,
            node_url,
        } => {
            let transaction: Transaction = serde_json::from_slice(&fs::read(transaction)?)?;
            let response = reqwest::Client::new()
                .post(format!("{node_url}/v1/transactions"))
                .json(&transaction)
                .send()
                .await?;
            let status = response.status();
            let body = response.text().await?;
            if !status.is_success() {
                return Err(format!("node rejected transaction ({status}): {body}").into());
            }
            println!("{body}");
        }
        Command::FeeEstimate {
            inputs,
            outputs,
            atoms_per_weight,
        } => {
            println!(
                "{}",
                Wallet::estimate_hybrid_fee(inputs, outputs, atoms_per_weight).atoms()
            );
        }
    }
    Ok(())
}

fn load(path: &Path, password: Option<&str>) -> Result<Wallet, Box<dyn std::error::Error>> {
    Ok(Wallet::load_encrypted(path, required_password(password)?)?)
}

fn required_password(password: Option<&str>) -> Result<&[u8], &'static str> {
    password
        .map(str::as_bytes)
        .ok_or("set QBTC_WALLET_PASSWORD or pass --password")
}

fn parse_network(value: &str) -> Result<Network, &'static str> {
    match value {
        "main" => Ok(Network::Main),
        "testnet" => Ok(Network::Testnet),
        "regtest" => Ok(Network::Regtest),
        _ => Err("chain must be main, testnet, or regtest"),
    }
}
