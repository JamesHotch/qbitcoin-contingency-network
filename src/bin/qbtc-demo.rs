use clap::{Parser, ValueEnum};
use qbitcoin::demo::{run_attack_demo, run_demo};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Mode {
    Run,
    Attack,
}

#[derive(Debug, Parser)]
#[command(about = "QBitCoin deterministic consensus demonstration")]
struct Arguments {
    #[arg(long, value_enum, default_value_t = Mode::Run)]
    mode: Mode,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = match Arguments::parse().mode {
        Mode::Run => run_demo()?,
        Mode::Attack => run_attack_demo()?,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
