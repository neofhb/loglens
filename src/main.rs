use clap::Parser;
use loglens::cli::{Cli, run};

fn main() {
    if let Err(err) = run(Cli::parse()) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
